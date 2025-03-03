use crate::{
	library::Library,
	location::{
		delete_location, find_location, indexer::rules::IndexerRuleCreateArgs, light_scan_location,
		location_with_indexer_rules, relink_location, scan_location, LocationCreateArgs,
		LocationError, LocationUpdateArgs,
	},
	prisma::{file_path, indexer_rule, indexer_rules_in_location, location, object, tag},
};

use std::path::PathBuf;

use rspc::{self, ErrorCode, RouterBuilderLike, Type};
use serde::{Deserialize, Serialize};

use super::{utils::LibraryRequest, Ctx, RouterBuilder};

#[derive(Serialize, Deserialize, Type, Debug)]
#[serde(tag = "type")]
pub enum ExplorerContext {
	Location(location::Data),
	Tag(tag::Data),
	// Space(object_in_space::Data),
}

#[derive(Serialize, Deserialize, Type, Debug)]
#[serde(tag = "type")]
pub enum ExplorerItem {
	Path {
		// has_thumbnail is determined by the local existence of a thumbnail
		has_thumbnail: bool,
		item: file_path_with_object::Data,
	},
	Object {
		has_thumbnail: bool,
		item: object_with_file_paths::Data,
	},
}

#[derive(Serialize, Deserialize, Type, Debug)]
pub struct ExplorerData {
	pub context: ExplorerContext,
	pub items: Vec<ExplorerItem>,
}

file_path::include!(file_path_with_object { object });
object::include!(object_with_file_paths { file_paths });

pub(crate) fn mount() -> impl RouterBuilderLike<Ctx> {
	<RouterBuilder>::new()
		.library_query("list", |t| {
			t(|_, _: (), library| async move {
				Ok(library
					.db
					.location()
					.find_many(vec![])
					.include(location::include!({ node }))
					.exec()
					.await?)
			})
		})
		.library_query("getById", |t| {
			t(|_, location_id: i32, library| async move {
				Ok(library
					.db
					.location()
					.find_unique(location::id::equals(location_id))
					.include(location_with_indexer_rules::include())
					.exec()
					.await?)
			})
		})
		.library_query("getExplorerData", |t| {
			#[derive(Clone, Serialize, Deserialize, Type, Debug)]
			pub struct LocationExplorerArgs {
				pub location_id: i32,
				pub path: String,
				pub limit: i32,
				pub cursor: Option<String>,
			}

			t(|_, mut args: LocationExplorerArgs, library| async move {
				let Library { db, .. } = &library;

				let location = find_location(&library, args.location_id)
					.exec()
					.await?
					.ok_or(LocationError::IdNotFound(args.location_id))?;

				if !args.path.ends_with('/') {
					args.path += "/";
				}

				let directory = db
					.file_path()
					.find_first(vec![
						file_path::location_id::equals(location.id),
						file_path::materialized_path::equals(args.path),
						file_path::is_dir::equals(true),
					])
					.exec()
					.await?
					.ok_or_else(|| {
						rspc::Error::new(ErrorCode::NotFound, "Directory not found".into())
					})?;

				let file_paths = db
					.file_path()
					.find_many(vec![
						file_path::location_id::equals(location.id),
						file_path::parent_id::equals(Some(directory.id)),
					])
					.include(file_path_with_object::include())
					.exec()
					.await?;

				let mut items = Vec::with_capacity(file_paths.len());

				for file_path in file_paths {
					let has_thumbnail = if let Some(cas_id) = &file_path.cas_id {
						library
							.thumbnail_exists(cas_id)
							.await
							.map_err(LocationError::IOError)?
					} else {
						false
					};

					items.push(ExplorerItem::Path {
						has_thumbnail,
						item: file_path,
					});
				}

				Ok(ExplorerData {
					context: ExplorerContext::Location(location),
					items,
				})
			})
		})
		.library_mutation("create", |t| {
			t(|_, args: LocationCreateArgs, library| async move {
				let location = args.create(&library).await?;
				scan_location(&library, location).await?;
				Ok(())
			})
		})
		.library_mutation("update", |t| {
			t(|_, args: LocationUpdateArgs, library| async move {
				args.update(&library).await.map_err(Into::into)
			})
		})
		.library_mutation("delete", |t| {
			t(|_, location_id: i32, library| async move {
				delete_location(&library, location_id)
					.await
					.map_err(Into::into)
			})
		})
		.library_mutation("relink", |t| {
			t(|_, location_path: PathBuf, library| async move {
				relink_location(&library, location_path)
					.await
					.map_err(Into::into)
			})
		})
		.library_mutation("addLibrary", |t| {
			t(|_, args: LocationCreateArgs, library| async move {
				let location = args.add_library(&library).await?;
				scan_location(&library, location).await?;
				Ok(())
			})
		})
		.library_mutation("fullRescan", |t| {
			t(|_, location_id: i32, library| async move {
				// rescan location
				scan_location(
					&library,
					find_location(&library, location_id)
						.include(location_with_indexer_rules::include())
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(location_id))?,
				)
				.await
				.map_err(Into::into)
			})
		})
		.library_mutation("quickRescan", |t| {
			#[derive(Clone, Serialize, Deserialize, Type, Debug)]
			pub struct LightScanArgs {
				pub location_id: i32,
				pub sub_path: String,
			}

			t(|_, args: LightScanArgs, library| async move {
				// light rescan location
				light_scan_location(
					&library,
					find_location(&library, args.location_id)
						.include(location_with_indexer_rules::include())
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(args.location_id))?,
					&args.sub_path,
				)
				.await
				.map_err(Into::into)
			})
		})
		.subscription("online", |t| {
			t(|ctx, _: ()| {
				let location_manager = ctx.library_manager.node_context.location_manager.clone();

				let mut rx = location_manager.online_rx();

				async_stream::stream! {
					let online = location_manager.get_online().await;
					// dbg!(&online);
					yield online;

					while let Ok(locations) = rx.recv().await {
						yield locations;
					}
				}
			})
		})
		.merge("indexer_rules.", mount_indexer_rule_routes())
}

fn mount_indexer_rule_routes() -> RouterBuilder {
	<RouterBuilder>::new()
		.library_mutation("create", |t| {
			t(|_, args: IndexerRuleCreateArgs, library| async move {
				args.create(&library).await.map_err(Into::into)
			})
		})
		.library_mutation("delete", |t| {
			t(|_, indexer_rule_id: i32, library| async move {
				library
					.db
					.indexer_rules_in_location()
					.delete_many(vec![indexer_rules_in_location::indexer_rule_id::equals(
						indexer_rule_id,
					)])
					.exec()
					.await?;

				library
					.db
					.indexer_rule()
					.delete(indexer_rule::id::equals(indexer_rule_id))
					.exec()
					.await?;

				Ok(())
			})
		})
		.library_query("get", |t| {
			t(|_, indexer_rule_id: i32, library| async move {
				library
					.db
					.indexer_rule()
					.find_unique(indexer_rule::id::equals(indexer_rule_id))
					.exec()
					.await?
					.ok_or_else(|| {
						rspc::Error::new(
							ErrorCode::NotFound,
							format!("Indexer rule <id={indexer_rule_id}> not found"),
						)
					})
			})
		})
		.library_query("list", |t| {
			t(|_, _: (), library| async move {
				library
					.db
					.indexer_rule()
					.find_many(vec![])
					.exec()
					.await
					.map_err(Into::into)
			})
		})
		// list indexer rules for location, returning the indexer rule
		.library_query("listForLocation", |t| {
			t(|_, location_id: i32, library| async move {
				library
					.db
					.indexer_rule()
					.find_many(vec![indexer_rule::locations::some(vec![
						indexer_rules_in_location::location_id::equals(location_id),
					])])
					.exec()
					.await
					.map_err(Into::into)
			})
		})
}
