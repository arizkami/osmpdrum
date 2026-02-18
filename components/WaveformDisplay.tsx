import React, { useRef, useEffect, useState } from 'react';
import * as d3 from 'd3';

interface WaveformProps {
    color: string;
    audioBuffer?: AudioBuffer;
    onFileLoad?: (buffer: AudioBuffer, fileName: string) => void;
    isPlaying?: boolean;
    playProgress?: number;
}

export const WaveformDisplay: React.FC<WaveformProps> = ({ color, audioBuffer, onFileLoad, isPlaying = false, playProgress = 0 }) => {
    const svgRef = useRef<SVGSVGElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const playheadRef = useRef<SVGLineElement>(null);
    const [isDragging, setIsDragging] = useState(false);
    const [audioContext] = useState(() => new (window.AudioContext || (window as any).webkitAudioContext)());
    const [dimensions, setDimensions] = useState({ width: 800, height: 96 });

    // Update dimensions on mount and resize
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
        window.addEventListener('resize', updateDimensions);
        return () => window.removeEventListener('resize', updateDimensions);
    }, []);

    // Process audio buffer and draw waveform with d3
    useEffect(() => {
        if (!audioBuffer || !svgRef.current || dimensions.width === 0) return;

        const svg = d3.select(svgRef.current);
        svg.selectAll('*').remove();

        const { width, height } = dimensions;
        
        // Get audio data from first channel
        const rawData = audioBuffer.getChannelData(0);
        const samples = Math.min(500, width); // Adaptive sample count
        const blockSize = Math.floor(rawData.length / samples);
        const filteredData: number[] = [];

        // Downsample audio data
        for (let i = 0; i < samples; i++) {
            let blockStart = blockSize * i;
            let sum = 0;
            for (let j = 0; j < blockSize; j++) {
                sum += Math.abs(rawData[blockStart + j]);
            }
            filteredData.push(sum / blockSize);
        }

        // Normalize data
        const max = Math.max(...filteredData, 0.001);
        const normalizedData = filteredData.map(n => (n / max) * 0.9); // Scale to 90% of height

        // Create scales
        const xScale = d3.scaleLinear()
            .domain([0, samples - 1])
            .range([0, width]);

        const yScale = d3.scaleLinear()
            .domain([0, 1])
            .range([height / 2, 0]);

        // Create area generator for filled waveform
        const area = d3.area<number>()
            .x((_, i) => xScale(i))
            .y0(height / 2)
            .y1(d => yScale(d))
            .curve(d3.curveBasis);

        // Draw top half (positive)
        svg.append('path')
            .datum(normalizedData)
            .attr('d', area)
            .attr('fill', 'currentColor')
            .attr('opacity', 0.6);

        // Draw bottom half (negative/mirrored)
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

        // Draw center line
        svg.append('line')
            .attr('x1', 0)
            .attr('x2', width)
            .attr('y1', height / 2)
            .attr('y2', height / 2)
            .attr('stroke', 'currentColor')
            .attr('stroke-width', 1)
            .attr('opacity', 0.3);

        // Add playhead line
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

    }, [audioBuffer, dimensions]);

    // Update playhead position
    useEffect(() => {
        if (playheadRef.current && dimensions.width > 0) {
            const x = playProgress * dimensions.width;
            d3.select(playheadRef.current)
                .attr('x1', x)
                .attr('x2', x)
                .attr('opacity', isPlaying ? 1 : 0);
        }
    }, [playProgress, isPlaying, dimensions.width]);

    // Handle file drop
    const handleDrop = async (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        setIsDragging(false);

        const files = Array.from(e.dataTransfer.files);
        const audioFile = files.find(f => 
            f.type.startsWith('audio/') || 
            f.name.endsWith('.wav') || 
            f.name.endsWith('.mp3') || 
            f.name.endsWith('.ogg')
        );

        if (audioFile && onFileLoad) {
            try {
                const arrayBuffer = await audioFile.arrayBuffer();
                const buffer = await audioContext.decodeAudioData(arrayBuffer);
                onFileLoad(buffer, audioFile.name);
            } catch (error) {
                console.error('Error loading audio file:', error);
                alert('Failed to load audio file. Make sure it\'s a valid audio format.');
            }
        }
    };

    const handleDragOver = (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        setIsDragging(true);
    };

    const handleDragLeave = (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        setIsDragging(false);
    };

    return (
        <div 
            ref={containerRef}
            className={`flex-1 relative flex items-center justify-center p-4 bg-[#0a0a0a] transition-all ${isDragging ? 'ring-2 ring-inset ring-cyan-500 bg-cyan-500/10' : ''}`}
            onDrop={handleDrop}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
        >
            {/* Grid Lines */}
            <div className="absolute inset-0 grid grid-cols-4 grid-rows-2 pointer-events-none opacity-20">
                <div className="border-r border-gray-600 h-full"></div>
                <div className="border-r border-gray-600 h-full"></div>
                <div className="border-r border-gray-600 h-full"></div>
                <div className="col-span-4 border-b border-gray-600 w-full"></div>
            </div>

            {!audioBuffer && (
                <div className="absolute inset-0 flex items-center justify-center text-text-muted text-xs pointer-events-none">
                    <div className="text-center">
                        <div className="mb-1">Drop audio file here</div>
                        <div className="text-[10px] opacity-50">.wav, .mp3, .ogg</div>
                    </div>
                </div>
            )}

            <svg 
                ref={svgRef}
                className={`w-full h-24 ${color} opacity-90`}
                width={dimensions.width}
                height={dimensions.height}
                preserveAspectRatio="none"
            />
        </div>
    );
};
