import { RadioGroup } from '@headlessui/react';
import { Info } from 'phosphor-react';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Tooltip } from '@sd/ui';
import { PasswordInput, Switch, useZodForm, z } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/components/AlertDialog';
import { usePlatform } from '~/util/Platform';

const schema = z.object({
	type: z.union([z.literal('password'), z.literal('key')]),
	outputPath: z.string(),
	mountAssociatedKey: z.boolean(),
	password: z.string(),
	saveToKeyManager: z.boolean()
});

interface Props extends UseDialogProps {
	location_id: number;
	path_id: number;
}

export default (props: Props) => {
	const platform = usePlatform();
	const dialog = useDialog(props);

	const mountedUuids = useLibraryQuery(['keys.listMounted'], {
		onSuccess: (data) => {
			hasMountedKeys = data.length > 0 ? true : false;
			if (!hasMountedKeys) {
				form.setValue('type', 'password');
			} else {
				form.setValue('type', 'key');
			}
		}
	});

	let hasMountedKeys =
		mountedUuids.data !== undefined && mountedUuids.data.length > 0 ? true : false;

	const decryptFile = useLibraryMutation('files.decryptFiles', {
		onSuccess: () => {
			showAlertDialog({
				title: 'Success',
				value:
					'The decryption job has started successfully. You may track the progress in the job overview panel.'
			});
		},
		onError: () => {
			showAlertDialog({
				title: 'Error',
				value: 'The decryption job failed to start.'
			});
		}
	});

	const form = useZodForm({
		defaultValues: {
			type: hasMountedKeys ? 'key' : 'password',
			saveToKeyManager: true,
			outputPath: '',
			password: '',
			mountAssociatedKey: true
		},
		schema
	});

	const onSubmit = form.handleSubmit((data) =>
		decryptFile.mutateAsync({
			location_id: props.location_id,
			path_id: props.path_id,
			output_path: data.outputPath !== '' ? data.outputPath : null,
			mount_associated_key: data.mountAssociatedKey,
			password: data.type === 'password' ? data.password : null,
			save_to_library: data.type === 'password' ? data.saveToKeyManager : null
		})
	);

	return (
		<Dialog
			form={form}
			dialog={dialog}
			onSubmit={onSubmit}
			title="Decrypt a file"
			description="Leave the output file blank for the default."
			loading={decryptFile.isLoading}
			ctaLabel="Decrypt"
		>
			<div className="space-y-2 py-2">
				<h2 className="text-xs font-bold">Key Type</h2>
				<RadioGroup
					value={form.watch('type')}
					onChange={(e: 'key' | 'password') => form.setValue('type', e)}
					className="mt-2 flex flex-row gap-2"
				>
					<RadioGroup.Option disabled={!hasMountedKeys} value="key">
						{({ checked }) => (
							<Button
								type="button"
								disabled={!hasMountedKeys}
								size="sm"
								variant={checked ? 'accent' : 'gray'}
							>
								Key Manager
							</Button>
						)}
					</RadioGroup.Option>
					<RadioGroup.Option value="password">
						{({ checked }) => (
							<Button type="button" size="sm" variant={checked ? 'accent' : 'gray'}>
								Password
							</Button>
						)}
					</RadioGroup.Option>
				</RadioGroup>

				{form.watch('type') === 'key' && (
					<div className="flex flex-row items-center">
						<Switch
							className="bg-app-selected"
							size="sm"
							name=""
							checked={form.watch('mountAssociatedKey')}
							onCheckedChange={(e) => form.setValue('mountAssociatedKey', e)}
						/>
						<span className="ml-3 mt-0.5 text-xs font-medium">Automatically mount key</span>
						<Tooltip label="The key linked with the file will be automatically mounted">
							<Info className="text-ink-faint ml-1.5 mt-0.5 h-4 w-4" />
						</Tooltip>
					</div>
				)}

				{form.watch('type') === 'password' && (
					<>
						<PasswordInput
							placeholder="Password"
							size="sm"
							{...form.register('password', { required: true })}
						/>

						<div className="flex flex-row items-center">
							<Switch
								className="bg-app-selected"
								size="sm"
								{...form.register('saveToKeyManager')}
							/>
							<span className="ml-3 mt-0.5 text-xs font-medium">Save to Key Manager</span>
							<Tooltip label="This key will be saved to the key manager">
								<Info className="text-ink-faint ml-1.5 mt-0.5 h-4 w-4" />
							</Tooltip>
						</div>
					</>
				)}

				<h2 className="text-xs font-bold">Output file</h2>
				<Button
					size="sm"
					variant={form.watch('outputPath') !== '' ? 'accent' : 'gray'}
					className="h-[23px] text-xs leading-3"
					type="button"
					onClick={() => {
						// if we allow the user to encrypt multiple files simultaneously, this should become a directory instead
						if (!platform.saveFilePickerDialog) {
							// TODO: Support opening locations on web
							showAlertDialog({
								title: 'Error',
								value: "System dialogs aren't supported on this platform."
							});
							return;
						}
						platform.saveFilePickerDialog().then((result) => {
							if (result) form.setValue('outputPath', result as string);
						});
					}}
				>
					Select
				</Button>
			</div>
		</Dialog>
	);
};
