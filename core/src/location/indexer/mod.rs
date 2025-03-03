use crate::{
	invalidate_query,
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	library::Library,
	prisma::file_path,
	sync,
};

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
	time::Duration,
};

use chrono::{DateTime, Utc};
use int_enum::IntEnumError;
use rmp_serde::{decode, encode};
use rspc::ErrorCode;
use rules::RuleKind;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tokio::io;
use tracing::info;

use super::{
	file_path_helper::{FilePathError, MaterializedPath},
	location_with_indexer_rules,
};

pub mod indexer_job;
pub mod rules;
pub mod shallow_indexer_job;
mod walk;

/// `IndexerJobInit` receives a `location::Data` object to be indexed
/// and possibly a `sub_path` to be indexed. The `sub_path` is used when
/// we want do index just a part of a location.
#[derive(Serialize, Deserialize)]
pub struct IndexerJobInit {
	pub location: location_with_indexer_rules::Data,
	pub sub_path: Option<PathBuf>,
}

impl Hash for IndexerJobInit {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}
/// `IndexerJobData` contains the state of the indexer job, which includes a `location_path` that
/// is cached and casted on `PathBuf` from `local_path` column in the `location` table. It also
/// contains some metadata for logging purposes.
#[derive(Serialize, Deserialize)]
pub struct IndexerJobData {
	db_write_start: DateTime<Utc>,
	scan_read_time: Duration,
	total_paths: usize,
	indexed_paths: i64,
}

/// `IndexerJobStep` is a type alias, specifying that each step of the [`IndexerJob`] is a vector of
/// `IndexerJobStepEntry`. The size of this vector is given by the [`BATCH_SIZE`] constant.
pub type IndexerJobStep = Vec<IndexerJobStepEntry>;

/// `IndexerJobStepEntry` represents a single file to be indexed, given its metadata to be written
/// on the `file_path` table in the database
#[derive(Serialize, Deserialize)]
pub struct IndexerJobStepEntry {
	full_path: PathBuf,
	materialized_path: MaterializedPath,
	created_at: DateTime<Utc>,
	file_id: i32,
	parent_id: Option<i32>,
}

impl IndexerJobData {
	fn on_scan_progress(ctx: &WorkerContext, progress: Vec<ScanProgress>) {
		ctx.progress_debounced(
			progress
				.iter()
				.map(|p| match p.clone() {
					ScanProgress::ChunkCount(c) => JobReportUpdate::TaskCount(c),
					ScanProgress::SavedChunks(p) => JobReportUpdate::CompletedTaskCount(p),
					ScanProgress::Message(m) => JobReportUpdate::Message(m),
				})
				.collect(),
		)
	}
}

#[derive(Clone)]
pub enum ScanProgress {
	ChunkCount(usize),
	SavedChunks(usize),
	Message(String),
}

/// Error type for the indexer module
#[derive(Error, Debug)]
pub enum IndexerError {
	// Not Found errors
	#[error("Indexer rule not found: <id={0}>")]
	IndexerRuleNotFound(i32),

	// User errors
	#[error("Invalid indexer rule kind integer: {0}")]
	InvalidRuleKindInt(#[from] IntEnumError<RuleKind>),
	#[error("Glob builder error: {0}")]
	GlobBuilderError(#[from] globset::Error),

	// Internal Errors
	#[error("Database error: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("I/O error: {0}")]
	IOError(#[from] io::Error),
	#[error("Indexer rule parameters json serialization error: {0}")]
	RuleParametersSerdeJson(#[from] serde_json::Error),
	#[error("Indexer rule parameters encode error: {0}")]
	RuleParametersRMPEncode(#[from] encode::Error),
	#[error("Indexer rule parameters decode error: {0}")]
	RuleParametersRMPDecode(#[from] decode::Error),
	#[error("File path related error (error: {0})")]
	FilePathError(#[from] FilePathError),
}

impl From<IndexerError> for rspc::Error {
	fn from(err: IndexerError) -> Self {
		match err {
			IndexerError::IndexerRuleNotFound(_) => {
				rspc::Error::with_cause(ErrorCode::NotFound, err.to_string(), err)
			}

			IndexerError::InvalidRuleKindInt(_) | IndexerError::GlobBuilderError(_) => {
				rspc::Error::with_cause(ErrorCode::BadRequest, err.to_string(), err)
			}

			_ => rspc::Error::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}

async fn execute_indexer_step(
	location: &location_with_indexer_rules::Data,
	step: &[IndexerJobStepEntry],
	ctx: WorkerContext,
) -> Result<i64, JobError> {
	let Library { sync, db, .. } = &ctx.library;

	let (sync_stuff, paths): (Vec<_>, Vec<_>) = step
		.iter()
		.map(|entry| {
			let MaterializedPath {
				materialized_path,
				is_dir,
				name,
				extension,
				..
			} = entry.materialized_path.clone();

			use file_path::*;

			(
				sync.unique_shared_create(
					sync::file_path::SyncId {
						id: entry.file_id,
						location: sync::location::SyncId {
							pub_id: location.pub_id.clone(),
						},
					},
					[
						("materialized_path", json!(materialized_path.clone())),
						("name", json!(name.clone())),
						("is_dir", json!(is_dir)),
						("extension", json!(extension.clone())),
						("parent_id", json!(entry.parent_id)),
						("date_created", json!(entry.created_at)),
					],
				),
				file_path::create_unchecked(
					entry.file_id,
					location.id,
					materialized_path,
					name,
					extension,
					vec![
						is_dir::set(is_dir),
						parent_id::set(entry.parent_id),
						date_created::set(entry.created_at.into()),
					],
				),
			)
		})
		.unzip();

	let count = sync
		.write_ops(
			db,
			(
				sync_stuff,
				db.file_path().create_many(paths).skip_duplicates(),
			),
		)
		.await?;

	info!("Inserted {count} records");

	Ok(count)
}

fn finalize_indexer<SJob, Init>(
	location_path: impl AsRef<Path>,
	state: &JobState<SJob>,
	ctx: WorkerContext,
) -> JobResult
where
	SJob: StatefulJob<Init = Init, Data = IndexerJobData, Step = IndexerJobStep>,
	Init: Serialize + DeserializeOwned + Send + Sync + Hash,
{
	let data = state
		.data
		.as_ref()
		.expect("critical error: missing data on job state");

	tracing::info!(
		"scan of {} completed in {:?}. {} new files found, \
			indexed {} files in db. db write completed in {:?}",
		location_path.as_ref().display(),
		data.scan_read_time,
		data.total_paths,
		data.indexed_paths,
		(Utc::now() - data.db_write_start)
			.to_std()
			.expect("critical error: non-negative duration"),
	);

	if data.indexed_paths > 0 {
		invalidate_query!(ctx.library, "locations.getExplorerData");
	}

	Ok(Some(serde_json::to_value(state)?))
}
