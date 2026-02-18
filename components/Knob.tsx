import React, { useState, useEffect, useRef } from 'react';
import { KnobProps } from '../types';

export const Knob: React.FC<KnobProps> = ({ 
    value, 
    label, 
    onChange, 
    min = 0, 
    max = 100, 
    suffix = '' 
}) => {
    const [isDragging, setIsDragging] = useState(false);
    const startY = useRef<number>(0);
    const startValue = useRef<number>(0);

    const handleMouseDown = (e: React.MouseEvent) => {
        setIsDragging(true);
        startY.current = e.clientY;
        startValue.current = value;
    };

    useEffect(() => {
        const handleMouseMove = (e: MouseEvent) => {
            if (!isDragging) return;
            const delta = startY.current - e.clientY;
            const sensitivity = 0.5;
            let newValue = startValue.current + (delta * sensitivity);
            
            // Clamp
            if (newValue > max) newValue = max;
            if (newValue < min) newValue = min;
            
            onChange(newValue);
        };

        const handleMouseUp = () => {
            setIsDragging(false);
        };

        if (isDragging) {
            document.addEventListener('mousemove', handleMouseMove);
            document.addEventListener('mouseup', handleMouseUp);
            document.body.style.cursor = 'ns-resize';
        } else {
            document.body.style.cursor = 'default';
        }

        return () => {
            document.removeEventListener('mousemove', handleMouseMove);
            document.removeEventListener('mouseup', handleMouseUp);
            document.body.style.cursor = 'default';
        };
    }, [isDragging, max, min, onChange]);

    // Calculate rotation angle (270 degrees range, starting from -135 to +135)
    const percentage = (value - min) / (max - min);
    const angle = -135 + (percentage * 270);

    return (
        <div 
            className="flex flex-col items-center gap-1 cursor-ns-resize group select-none relative"
            onMouseDown={handleMouseDown}
        >
            <span className={`text-[11px] font-medium transition-colors ${isDragging ? 'text-accent-cyan' : 'text-text-muted group-hover:text-text-main'}`}>
                {label}
            </span>
            
            <div className="relative w-12 h-12">
                {/* Background circle */}
                <div className="absolute inset-0 rounded-full border-4 border-border-dark"></div>
                
                {/* Knob indicator */}
                <div 
                    className="absolute inset-0 transition-transform"
                    style={{ transform: `rotate(${angle}deg)` }}
                >
                    <div className={`absolute top-1 left-1/2 w-1 h-4 -ml-0.5 rounded-full transition-colors ${isDragging ? 'bg-accent-cyan' : 'bg-white'}`}></div>
                </div>
                
                {/* Center dot */}
                <div className="absolute inset-0 flex items-center justify-center">
                    <div className="w-2 h-2 rounded-full bg-gray-700"></div>
                </div>
            </div>

            <span className="text-[10px] font-bold font-mono text-text-main">
                {Math.floor(value)}{suffix}
            </span>
        </div>
    );
};