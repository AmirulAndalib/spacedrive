import { useClientContext, useLibraryMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { Input, useZodForm, z } from '@sd/ui/src/forms';
import ColorPicker from '~/components/ColorPicker';
import { usePlatform } from '~/util/Platform';

export default (props: UseDialogProps) => {
	const dialog = useDialog(props);
	const platform = usePlatform();
	const submitPlausibleEvent = usePlausibleEvent({ platformType: platform.platform });

	const form = useZodForm({
		schema: z.object({
			name: z.string(),
			color: z.string()
		}),
		defaultValues: {
			color: '#A717D9'
		}
	});

	const createTag = useLibraryMutation('tags.create', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagCreate' } });
		},
		onError: (e) => {
			console.error('error', e);
		}
	});

	return (
		<Dialog
			{...{ dialog, form }}
			onSubmit={form.handleSubmit((data) => createTag.mutateAsync(data))}
			title="Create New Tag"
			description="Choose a name and color."
			ctaLabel="Create"
		>
			<div className="relative mt-3 ">
				<ColorPicker className="!absolute left-[9px] top-[-5px]" {...form.register('color')} />
				<Input
					{...form.register('name', { required: true })}
					className="w-full pl-[40px]"
					placeholder="Name"
				/>
			</div>
		</Dialog>
	);
};
