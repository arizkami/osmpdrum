import React, { useEffect, useMemo, useRef } from 'react';
import * as d3 from 'd3';

interface VUMeterProps {
    width?: number;
    height?: number;
    level: number; // 0..1
}

export const VUMeter: React.FC<VUMeterProps> = ({ width = 54, height = 54, level }) => {
    const svgRef = useRef<SVGSVGElement>(null);

    const clamped = useMemo(() => {
        if (!Number.isFinite(level)) return 0;
        return Math.max(0, Math.min(1, level));
    }, [level]);

    useEffect(() => {
        const svg = d3.select(svgRef.current);
        if (svg.empty()) return;

        svg.attr('width', width).attr('height', height).attr('viewBox', `0 0 ${width} ${height}`);

        const padding = 6;
        const innerW = width - padding * 2;
        const innerH = height - padding * 2;

        const root = svg.selectAll('g.root').data([0]);
        const rootEnter = root.enter().append('g').attr('class', 'root');
        const g = rootEnter.merge(root as any).attr('transform', `translate(${padding}, ${padding})`);

        const barCount = 14;
        const gap = 2;
        const barH = (innerH - gap * (barCount - 1)) / barCount;

        const yForIndex = (i: number) => innerH - (i + 1) * barH - i * gap;

        const thresholds = d3.range(0, barCount).map(i => (i + 1) / barCount);

        const colorFor = (t: number) => {
            if (t <= 0.65) return '#12e3ff';
            if (t <= 0.85) return '#f5c542';
            return '#ff3b3b';
        };

        const bars = g
            .selectAll<SVGRectElement, number>('rect.bar')
            .data(thresholds, (d: any) => String(d));

        bars
            .enter()
            .append('rect')
            .attr('class', 'bar')
            .attr('rx', 1)
            .attr('ry', 1)
            .attr('x', 0)
            .attr('width', innerW)
            .attr('y', (_d, i) => yForIndex(i))
            .attr('height', barH)
            .style('fill', (d) => colorFor(d))
            .style('opacity', 0.15)
            .merge(bars)
            .transition()
            .duration(80)
            .ease(d3.easeCubicOut)
            .style('opacity', (d) => (clamped >= d ? 1 : 0.15));

        bars.exit().remove();

        const outline = g.selectAll('rect.outline').data([0]);
        outline
            .enter()
            .append('rect')
            .attr('class', 'outline')
            .attr('x', -1)
            .attr('y', -1)
            .attr('width', innerW + 2)
            .attr('height', innerH + 2)
            .attr('rx', 3)
            .attr('ry', 3)
            .style('fill', 'none')
            .style('stroke', '#2a2a2a')
            .style('stroke-width', 1);
    }, [clamped, height, width]);

    return <svg ref={svgRef} />;
};
