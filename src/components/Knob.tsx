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

    const percentage = (value - min) / (max - min);

    // SVG arc knob geometry
    const size = 52;
    const cx = size / 2;
    const cy = size / 2;
    const r = 19;

    const polarToXY = (angleDeg: number, radius: number) => {
        const rad = ((angleDeg - 90) * Math.PI) / 180;
        return {
            x: +(cx + radius * Math.cos(rad)).toFixed(2),
            y: +(cy + radius * Math.sin(rad)).toFixed(2),
        };
    };

    const startAngle = -135;
    const valueAngle = startAngle + percentage * 270;

    const trackStartPt = polarToXY(startAngle, r);
    const trackEndPt = polarToXY(135, r);
    const valueEndPt = polarToXY(valueAngle, r);

    const trackD = `M ${trackStartPt.x} ${trackStartPt.y} A ${r} ${r} 0 1 1 ${trackEndPt.x} ${trackEndPt.y}`;

    const valueDelta = valueAngle - startAngle;
    const valueLargeArc = valueDelta > 180 ? 1 : 0;
    const valueD = percentage > 0.005
        ? `M ${trackStartPt.x} ${trackStartPt.y} A ${r} ${r} 0 ${valueLargeArc} 1 ${valueEndPt.x} ${valueEndPt.y}`
        : null;

    const indicatorOuter = polarToXY(valueAngle, r - 5);
    const indicatorInner = polarToXY(valueAngle, r - 13);

    return (
        <div 
            className="flex flex-col items-center gap-0.5 cursor-ns-resize group select-none"
            onMouseDown={handleMouseDown}
        >
            <span className={`text-[11px] font-medium mb-0.5 transition-colors ${isDragging ? 'text-accent-cyan' : 'text-text-muted group-hover:text-text-main'}`}>
                {label}
            </span>

            <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
                {/* Track arc (dim full range) */}
                <path
                    d={trackD}
                    fill="none"
                    stroke="#2a2a2a"
                    strokeWidth="3"
                    strokeLinecap="round"
                />
                {/* Value arc (lit) */}
                {valueD && (
                    <path
                        d={valueD}
                        fill="none"
                        stroke={isDragging ? '#7df9ff' : '#3a8a9a'}
                        strokeWidth="3"
                        strokeLinecap="round"
                        style={{ filter: isDragging ? 'drop-shadow(0 0 4px #7df9ff)' : undefined }}
                    />
                )}
                {/* Knob body */}
                <circle
                    cx={cx} cy={cy} r={10}
                    fill="#181818"
                    stroke={isDragging ? '#7df9ff44' : '#2e2e2e'}
                    strokeWidth="1"
                />
                {/* Indicator line */}
                <line
                    x1={indicatorInner.x} y1={indicatorInner.y}
                    x2={indicatorOuter.x} y2={indicatorOuter.y}
                    stroke={isDragging ? '#7df9ff' : '#bbbbbb'}
                    strokeWidth="1.5"
                    strokeLinecap="round"
                />
            </svg>

            <span className={`text-[10px] font-bold font-mono mt-0.5 transition-colors ${isDragging ? 'text-accent-cyan' : 'text-text-main'}`}>
                {Math.floor(value)}{suffix}
            </span>
        </div>
    );
};