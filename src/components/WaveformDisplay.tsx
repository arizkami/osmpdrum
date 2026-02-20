import React, { useRef, useEffect, useState } from 'react';
import * as d3 from 'd3';

interface WaveformProps {
    color: string;
    peaks?: number[];
    isPlaying?: boolean;
    playProgress?: number;
    startPoint?: number; // 0-1 range
    endPoint?: number; // 0-1 range
    onStartPointChange?: (value: number) => void;
    onEndPointChange?: (value: number) => void;
}

export const WaveformDisplay: React.FC<WaveformProps> = ({ 
    color, 
    peaks, 
    isPlaying = false, 
    playProgress = 0,
    startPoint = 0,
    endPoint = 1,
    onStartPointChange,
    onEndPointChange
}) => {
    const svgRef = useRef<SVGSVGElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const playheadRef = useRef<SVGLineElement>(null);
    const startMarkerRef = useRef<SVGGElement>(null);
    const endMarkerRef = useRef<SVGGElement>(null);
    const [dimensions, setDimensions] = useState({ width: 800, height: 96 });
    const [draggingMarker, setDraggingMarker] = useState<'start' | 'end' | null>(null);

    useEffect(() => {
        const updateDimensions = () => {
            if (containerRef.current) {
                setDimensions({
                    width: containerRef.current.clientWidth,
                    height: 96
                });
            }
        };

        updateDimensions();
        // Delay slightly to ensure layout is done
        setTimeout(updateDimensions, 100);
        window.addEventListener('resize', updateDimensions);
        return () => window.removeEventListener('resize', updateDimensions);
    }, []);

    useEffect(() => {
        if (!peaks || peaks.length === 0 || !svgRef.current || dimensions.width === 0) return;

        const svg = d3.select(svgRef.current);
        svg.selectAll('*').remove();

        const { width, height } = dimensions;
        const data = peaks;

        const max = Math.max(...data, 0.001);
        const normalizedData = data.map(n => (n / max) * 0.9);

        const xScale = d3.scaleLinear()
            .domain([0, data.length - 1])
            .range([0, width]);

        const yScale = d3.scaleLinear()
            .domain([0, 1])
            .range([height / 2, 0]);

        const area = d3.area<number>()
            .x((_, i) => xScale(i))
            .y0(height / 2)
            .y1(d => yScale(d))
            .curve(d3.curveBasis);

        svg.append('path')
            .datum(normalizedData)
            .attr('d', area)
            .attr('fill', 'currentColor')
            .attr('opacity', 0.6);

        const areaMirror = d3.area<number>()
            .x((_, i) => xScale(i))
            .y0(height / 2)
            .y1(d => height / 2 + (height / 2 - yScale(d)))
            .curve(d3.curveBasis);

        svg.append('path')
            .datum(normalizedData)
            .attr('d', areaMirror)
            .attr('fill', 'currentColor')
            .attr('opacity', 0.6);

        svg.append('line')
            .attr('x1', 0)
            .attr('x2', width)
            .attr('y1', height / 2)
            .attr('y2', height / 2)
            .attr('stroke', 'currentColor')
            .attr('stroke-width', 1)
            .attr('opacity', 0.3);

        const playhead = svg.append('line')
            .attr('class', 'playhead')
            .attr('x1', 0)
            .attr('x2', 0)
            .attr('y1', 0)
            .attr('y2', height)
            .attr('stroke', '#ff0080')
            .attr('stroke-width', 2)
            .attr('opacity', 0);

        playheadRef.current = playhead.node();

        // Start marker
        const startMarker = svg.append('g')
            .attr('class', 'start-marker')
            .attr('cursor', 'ew-resize');

        startMarker.append('line')
            .attr('x1', 0)
            .attr('x2', 0)
            .attr('y1', 0)
            .attr('y2', height)
            .attr('stroke', '#00ff00')
            .attr('stroke-width', 2)
            .attr('opacity', 0.8);

        startMarker.append('polygon')
            .attr('points', '0,0 8,0 8,12 4,16 0,12')
            .attr('fill', '#00ff00')
            .attr('opacity', 0.9);

        startMarkerRef.current = startMarker.node();

        // End marker
        const endMarker = svg.append('g')
            .attr('class', 'end-marker')
            .attr('cursor', 'ew-resize');

        endMarker.append('line')
            .attr('x1', 0)
            .attr('x2', 0)
            .attr('y1', 0)
            .attr('y2', height)
            .attr('stroke', '#ff6600')
            .attr('stroke-width', 2)
            .attr('opacity', 0.8);

        endMarker.append('polygon')
            .attr('points', '0,0 8,0 8,12 4,16 0,12')
            .attr('fill', '#ff6600')
            .attr('opacity', 0.9);

        endMarkerRef.current = endMarker.node();

    }, [peaks, dimensions]);

    useEffect(() => {
        if (playheadRef.current && dimensions.width > 0) {
            const x = playProgress * dimensions.width;
            d3.select(playheadRef.current)
                .attr('x1', x)
                .attr('x2', x)
                .attr('opacity', isPlaying ? 1 : 0);
        }
    }, [playProgress, isPlaying, dimensions.width]);

    // Update marker positions
    useEffect(() => {
        if (startMarkerRef.current && dimensions.width > 0) {
            const x = startPoint * dimensions.width;
            d3.select(startMarkerRef.current)
                .attr('transform', `translate(${x}, 0)`);
        }
        if (endMarkerRef.current && dimensions.width > 0) {
            const x = endPoint * dimensions.width;
            d3.select(endMarkerRef.current)
                .attr('transform', `translate(${x}, 0)`);
        }
    }, [startPoint, endPoint, dimensions.width]);

    // Handle marker dragging
    useEffect(() => {
        const handleMouseMove = (e: MouseEvent) => {
            if (!draggingMarker || !containerRef.current) return;

            const rect = containerRef.current.getBoundingClientRect();
            const x = e.clientX - rect.left - 16; // Account for padding
            const normalizedX = Math.max(0, Math.min(1, x / (rect.width - 32)));

            if (draggingMarker === 'start' && onStartPointChange) {
                onStartPointChange(Math.min(normalizedX, endPoint - 0.01));
            } else if (draggingMarker === 'end' && onEndPointChange) {
                onEndPointChange(Math.max(normalizedX, startPoint + 0.01));
            }
        };

        const handleMouseUp = () => {
            setDraggingMarker(null);
        };

        if (draggingMarker) {
            window.addEventListener('mousemove', handleMouseMove);
            window.addEventListener('mouseup', handleMouseUp);
            return () => {
                window.removeEventListener('mousemove', handleMouseMove);
                window.removeEventListener('mouseup', handleMouseUp);
            };
        }
    }, [draggingMarker, startPoint, endPoint, onStartPointChange, onEndPointChange]);

    const handleMarkerMouseDown = (marker: 'start' | 'end') => (e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        setDraggingMarker(marker);
    };

    return (
        <div
            ref={containerRef}
            className={`flex-1 relative flex items-center justify-center p-4 bg-[#0a0a0a]`}
        >
            <div className="absolute inset-0 grid grid-cols-4 grid-rows-2 pointer-events-none opacity-20">
                <div className="border-r border-gray-600 h-full"></div>
                <div className="border-r border-gray-600 h-full"></div>
                <div className="border-r border-gray-600 h-full"></div>
                <div className="col-span-4 border-b border-gray-600 w-full"></div>
            </div>

            {!peaks && (
                <div className="absolute inset-0 flex items-center justify-center text-text-muted text-xs pointer-events-none">
                    <div className="text-center">
                        <div className="mb-1">Drop audio file here</div>
                    </div>
                </div>
            )}

            <svg
                ref={svgRef}
                className={`w-full h-24 ${color} opacity-90`}
                width={dimensions.width}
                height={dimensions.height}
                preserveAspectRatio="none"
                onMouseDown={(e) => {
                    const target = e.target as SVGElement;
                    if (target.closest('.start-marker')) {
                        handleMarkerMouseDown('start')(e);
                    } else if (target.closest('.end-marker')) {
                        handleMarkerMouseDown('end')(e);
                    }
                }}
            />
        </div>
    );
};
