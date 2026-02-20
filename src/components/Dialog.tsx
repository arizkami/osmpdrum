import React, { useEffect } from 'react';
import { X } from 'lucide-react';

interface DialogProps {
    isOpen: boolean;
    onClose: () => void;
    title?: string;
    children: React.ReactNode;
    showCloseButton?: boolean;
    size?: 'sm' | 'md' | 'lg';
}

export const Dialog: React.FC<DialogProps> = ({ 
    isOpen, 
    onClose, 
    title, 
    children, 
    showCloseButton = true,
    size = 'md'
}) => {
    useEffect(() => {
        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === 'Escape' && isOpen) {
                onClose();
            }
        };

        if (isOpen) {
            document.addEventListener('keydown', handleEscape);
            document.body.style.overflow = 'hidden';
        }

        return () => {
            document.removeEventListener('keydown', handleEscape);
            document.body.style.overflow = 'unset';
        };
    }, [isOpen, onClose]);

    if (!isOpen) return null;

    const sizeClasses = {
        sm: 'max-w-sm',
        md: 'max-w-md',
        lg: 'max-w-2xl'
    };

    return (
        <div 
            className="fixed inset-0 z-50 flex items-center justify-center p-4"
            onClick={onClose}
        >
            {/* Backdrop with blur */}
            <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />

            {/* Dialog */}
            <div 
                className={`relative ${sizeClasses[size]} w-full bg-gradient-to-br from-[#1a1a1a] to-[#0f0f0f] border border-border-dark rounded-lg shadow-2xl shadow-black/50 overflow-hidden`}
                onClick={(e) => e.stopPropagation()}
            >
                {/* Header */}
                {(title || showCloseButton) && (
                    <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-dark bg-header-bg">
                        {title && (
                            <h2 className="text-xs font-bold uppercase tracking-wider text-text-main">
                                {title}
                            </h2>
                        )}
                        {showCloseButton && (
                            <button
                                onClick={onClose}
                                className="ml-auto p-1 hover:bg-white/10 rounded transition-colors text-text-muted hover:text-text-main"
                            >
                                <X size={14} />
                            </button>
                        )}
                    </div>
                )}

                {/* Content */}
                <div className="p-4 text-xs text-text-main">
                    {children}
                </div>
            </div>
        </div>
    );
};

interface DialogFooterProps {
    children: React.ReactNode;
}

export const DialogFooter: React.FC<DialogFooterProps> = ({ children }) => {
    return (
        <div className="flex items-center justify-end gap-2 pt-3 mt-3 border-t border-border-dark">
            {children}
        </div>
    );
};

interface DialogButtonProps {
    onClick: () => void;
    children: React.ReactNode;
    variant?: 'primary' | 'secondary' | 'danger';
    disabled?: boolean;
}

export const DialogButton: React.FC<DialogButtonProps> = ({ 
    onClick, 
    children, 
    variant = 'secondary',
    disabled = false
}) => {
    const variantClasses = {
        primary: 'bg-accent-cyan text-black hover:bg-accent-cyan/90',
        secondary: 'bg-[#2a2a2a] text-text-main hover:bg-[#333]',
        danger: 'bg-red-600 text-white hover:bg-red-700'
    };

    return (
        <button
            onClick={onClick}
            disabled={disabled}
            className={`
                px-3 py-1.5 text-xs font-semibold rounded transition-colors
                ${variantClasses[variant]}
                ${disabled ? 'opacity-50 cursor-not-allowed' : ''}
            `}
        >
            {children}
        </button>
    );
};
