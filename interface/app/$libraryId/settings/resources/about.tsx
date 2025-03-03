import { AppLogo } from '@sd/assets/images';
import { useBridgeQuery } from '@sd/client';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

export const Component = () => {
	const buildInfo = useBridgeQuery(['buildInfo']);

	const os = useOperatingSystem();

	const currentPlatformNiceName =
		os === 'browser' ? 'Web' : os == 'macOS' ? os : os.charAt(0).toUpperCase() + os.slice(1);

	return (
		<>
			<div className="flex flex-row items-center">
				<img src={AppLogo} className="mr-8 h-[88px] w-[88px]" />
				<div className="flex flex-col">
					<h1 className="text-2xl font-bold">
						Spacedrive {os !== 'unknown' && <>for {currentPlatformNiceName}</>}
					</h1>
					<span className="text-ink-dull mt-1 text-sm">The file manager from the future.</span>
					<span className="text-ink-faint/80 mt-1 text-xs">
						v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
					</span>
				</div>
			</div>
		</>
	);
};
