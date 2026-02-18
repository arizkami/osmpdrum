import React, { useEffect, useRef } from 'react';

export interface ContextMenuItem {
    label: string;
    onClick: () => void;
    icon?: React.ReactNode;
    divider?: boolean;
    disabled?: boolean;
}

interface ContextMenuProps {
    x: number;
    y: number;
    items: ContextMenuItem[];
    onClose: () => void;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({ x, y, items, onClose }) => {
    const menuRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const handleClickOutside = (e: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
                onClose();
            }
        };

        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                onClose();
            }
        };

        document.addEventListener('mousedown', handleClickOutside);
        document.addEventListener('keydown', handleEscape);

        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
            document.removeEventListener('keydown', handleEscape);
        };
    }, [onClose]);

    return (
        <div
            ref={menuRef}
            className="fixed z-50 bg-panel-bg border border-border-dark rounded shadow-2xl py-1 min-w-[160px]"
            style={{
                left: `${x}px`,
                top: `${y}px`,
            }}
        >
            {items.map((item, index) => (
                item.divider ? (
                    <div key={index} className="h-px bg-border-dark my-1" />
                ) : (
                    <button
                        key={index}
                        onClick={() => {
                            if (!item.disabled) {
                                item.onClick();
                                onClose();
                            }
                        }}
                        disabled={item.disabled}
                        className={`
                            w-full px-3 py-1.5 text-left text-xs flex items-center gap-2
                            ${item.disabled 
                                ? 'text-text-muted opacity-50 cursor-not-allowed' 
                                : 'text-text-main hover:bg-accent-cyan/10 hover:text-accent-cyan cursor-pointer'
                            }
                            transition-colors
                        `}
                    >
                        {item.icon && <span className="w-4 h-4 flex items-center justify-center">{item.icon}</span>}
                        <span>{item.label}</span>
                    </button>
                )
            ))}
        </div>
    );
};
