import React, { useEffect, useMemo, useRef, useState } from 'react';

export interface MenuSelectItem {
    label: string;
    value: string;
    onSelect?: (value: string) => void;
    icon?: React.ReactNode;
    disabled?: boolean;
}

interface MenuSelectProps {
    value: string;
    placeholder: string;
    items: MenuSelectItem[];
    onChange: (value: string) => void;
}

export const MenuSelect: React.FC<MenuSelectProps> = ({ value, placeholder, items, onChange }) => {
    const triggerRef = useRef<HTMLButtonElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);
    const [isOpen, setIsOpen] = useState(false);
    const [pos, setPos] = useState<{ left: number; top: number; width: number }>({ left: 0, top: 0, width: 240 });

    const label = useMemo(() => {
        const match = items.find(i => i.value === value);
        return match?.label ?? (value || '');
    }, [items, value]);

    useEffect(() => {
        if (!isOpen) return;

        const updatePosition = () => {
            const rect = triggerRef.current?.getBoundingClientRect();
            if (!rect) return;
            setPos({ left: rect.left, top: rect.bottom + 6, width: rect.width });
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
        <div className="relative">
            <button
                ref={triggerRef}
                onClick={() => setIsOpen(v => !v)}
                className="w-full bg-[#111] border border-border-dark rounded px-2 py-1.5 text-xs flex items-center justify-between"
            >
                <span className={label ? 'text-text-main' : 'text-text-muted'}>{label || placeholder}</span>
                <span className="text-text-muted">▾</span>
            </button>

            {isOpen && (
                <div
                    ref={menuRef}
                    className="fixed z-50 bg-panel-bg border border-border-dark rounded shadow-2xl py-1"
                    style={{ left: `${pos.left}px`, top: `${pos.top}px`, minWidth: `${pos.width}px` }}
                >
                    {items.length === 0 && (
                        <div className="px-3 py-1.5 text-left text-xs text-text-muted opacity-70">(No items)</div>
                    )}
                    {items.map((item, index) => (
                        <button
                            key={`${item.value}-${index}`}
                            onClick={() => {
                                if (item.disabled) return;
                                onChange(item.value);
                                item.onSelect?.(item.value);
                                setIsOpen(false);
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
                    ))}
                </div>
            )}
        </div>
    );
};
