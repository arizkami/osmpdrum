import React, { useEffect, useMemo, useRef, useState } from 'react';

export interface DropdownMenuItem {
    label: string;
    onClick: () => void;
    icon?: React.ReactNode;
    divider?: boolean;
    disabled?: boolean;
}

interface DropdownMenuProps {
    label: string;
    items: DropdownMenuItem[];
    className?: string;
}

export const DropdownMenu: React.FC<DropdownMenuProps> = ({ label, items, className }) => {
    const triggerRef = useRef<HTMLButtonElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);
    const [isOpen, setIsOpen] = useState(false);
    const [pos, setPos] = useState<{ left: number; top: number; minWidth: number }>({ left: 0, top: 0, minWidth: 160 });

    const visibleItems = useMemo(() => items, [items]);

    useEffect(() => {
        if (!isOpen) return;

        const updatePosition = () => {
            const rect = triggerRef.current?.getBoundingClientRect();
            if (!rect) return;
            setPos({
                left: rect.left,
                top: rect.bottom + 6,
                minWidth: Math.max(160, Math.round(rect.width)),
            });
        };

        updatePosition();
        window.addEventListener('resize', updatePosition);
        window.addEventListener('scroll', updatePosition, true);
        return () => {
            window.removeEventListener('resize', updatePosition);
            window.removeEventListener('scroll', updatePosition, true);
        };
    }, [isOpen]);

    useEffect(() => {
        if (!isOpen) return;

        const handleClickOutside = (e: MouseEvent) => {
            const t = e.target as Node;
            if (menuRef.current?.contains(t)) return;
            if (triggerRef.current?.contains(t)) return;
            setIsOpen(false);
        };

        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === 'Escape') setIsOpen(false);
        };

        document.addEventListener('mousedown', handleClickOutside);
        document.addEventListener('keydown', handleEscape);
        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
            document.removeEventListener('keydown', handleEscape);
        };
    }, [isOpen]);

    return (
        <div className={`relative ${className ?? ''}`}>
            <button
                ref={triggerRef}
                onClick={() => setIsOpen(v => !v)}
                className={`hover:text-text-main transition-colors focus:outline-none ${isOpen ? 'text-text-main' : ''}`}
            >
                {label}
            </button>

            {isOpen && (
                <div
                    ref={menuRef}
                    className="fixed z-50 bg-panel-bg border border-border-dark rounded shadow-2xl py-1"
                    style={{ left: `${pos.left}px`, top: `${pos.top}px`, minWidth: `${pos.minWidth}px` }}
                >
                    {visibleItems.map((item, index) =>
                        item.divider ? (
                            <div key={index} className="h-px bg-border-dark my-1" />
                        ) : (
                            <button
                                key={index}
                                onClick={() => {
                                    if (!item.disabled) {
                                        item.onClick();
                                        setIsOpen(false);
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
                    )}
                </div>
            )}
        </div>
    );
};
