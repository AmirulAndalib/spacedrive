import * as RadixCM from '@radix-ui/react-context-menu';
import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { CaretRight, Icon } from 'phosphor-react';
import { PropsWithChildren, Suspense } from 'react';

interface Props extends RadixCM.MenuContentProps {
	trigger: React.ReactNode;
}

const MENU_CLASSES = `
  flex flex-col
  min-w-[8rem] p-1
  text-left text-sm dark:text-gray-100 text-gray-800
  bg-gray-50 border-gray-200 dark:bg-gray-650 dark:bg-opacity-80 backdrop-blur
	border border-transparent dark:border-gray-550
  shadow-md shadow-gray-300 dark:shadow-gray-750 
  select-none cursor-default rounded-md
`;

export const ContextMenu = ({
	trigger,
	children,
	className,
	...props
}: PropsWithChildren<Props>) => {
	return (
		<RadixCM.Root>
			<RadixCM.Trigger asChild>{trigger}</RadixCM.Trigger>
			<RadixCM.Portal>
				<RadixCM.Content {...props} className={clsx(MENU_CLASSES, className)}>
					{children}
				</RadixCM.Content>
			</RadixCM.Portal>
		</RadixCM.Root>
	);
};

export const Separator = () => (
	<RadixCM.Separator className="mx-2 border-0 border-b pointer-events-none border-b-gray-300 dark:border-b-gray-550" />
);

export const SubMenu = ({
	label,
	icon,
	className,
	...props
}: RadixCM.MenuSubContentProps & ItemProps) => {
	return (
		<RadixCM.Sub>
			<RadixCM.SubTrigger className="[&[data-state='open']_div]:bg-primary focus:outline-none  py-0.5">
				<DivItem rightArrow {...{ label, icon }} />
			</RadixCM.SubTrigger>
			<RadixCM.Portal>
				<Suspense fallback={null}>
					<RadixCM.SubContent {...props} className={clsx(MENU_CLASSES, '-mt-2', className)} />
				</Suspense>
			</RadixCM.Portal>
		</RadixCM.Sub>
	);
};

const ITEM_CLASSES = `
  flex flex-row items-center text-xs justify-start flex-1 
  px-2 py-[3px] space-x-2
  rounded
  
`;

const itemStyles = cva([ITEM_CLASSES], {
	variants: {
		variant: {
			default: 'group-hover:bg-primary focus:bg-primary',
			danger: `
        text-red-600 dark:text-red-400
        group-hover:text-white focus:text-white
        group-hover:bg-red-500 focus:bg-red-500
      `
		}
	},
	defaultVariants: {
		variant: 'default'
	}
});

interface ItemProps extends VariantProps<typeof itemStyles> {
	icon?: Icon;
	rightArrow?: boolean;
	label?: string;
	keybind?: string;
}

export const Item = ({
	icon,
	label,
	rightArrow,
	children,
	keybind,
	variant,
	...props
}: ItemProps & RadixCM.MenuItemProps) => (
	<RadixCM.Item
		{...props}
		className="!cursor-default select-none group focus:outline-none py-0.5 active:opacity-80"
	>
		<div className={itemStyles({ variant })}>
			{children ? children : <ItemInternals {...{ icon, label, rightArrow, keybind }} />}
		</div>
	</RadixCM.Item>
);

const DivItem = ({ variant, ...props }: ItemProps) => (
	<div className={itemStyles({ variant })}>
		<ItemInternals {...props} />
	</div>
);

const ItemInternals = ({ icon, label, rightArrow, keybind }: ItemProps) => {
	const ItemIcon = icon;
	return (
		<>
			{ItemIcon && <ItemIcon size={18} />}
			{label && <p>{label}</p>}

			{keybind && (
				<span className="absolute text-xs font-medium text-gray-500 right-3 flex-end dark:text-gray-400 group-hover:dark:text-white">
					{keybind}
				</span>
			)}
			{rightArrow && (
				<>
					<div className="flex-1" />
					<CaretRight weight="fill" size={12} alt="" />
				</>
			)}
		</>
	);
};
