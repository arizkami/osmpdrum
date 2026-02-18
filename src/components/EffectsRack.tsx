import React from 'react';
import { Plus } from 'lucide-react';

export const EffectsRack: React.FC = () => {
    return (
        <div className="h-48 bg-[#0c0c0c] p-2.5 flex gap-2 shrink-0 overflow-x-auto">
            
            <div className="w-48 flex flex-col border border-[#333] bg-panel-bg shrink-0">
                <div className="bg-[#1f1f1f] px-2.5 py-1 text-[11px] font-bold border-b border-[#333] text-gray-300 flex justify-between items-center">
                    <span>SATURATION</span>
                    <div className="w-2 h-2 rounded-full bg-accent-cyan shadow-[0_0_5px_#7df9ff]"></div>
                </div>
                <div className="flex-1 p-3 grid grid-cols-2 gap-2">
                    {/* Mock controls */}
                    <div className="w-full h-1 bg-[#333] rounded-full mt-4 relative">
                        <div className="absolute left-0 top-0 h-full w-3/4 bg-accent-cyan"></div>
                        <div className="absolute left-3/4 top-1/2 -translate-y-1/2 w-3 h-3 bg-white rounded-full shadow-md"></div>
                        <span className="absolute -top-4 left-0 text-[9px] text-gray-500">Drive</span>
                    </div>
                </div>
            </div>

            <div className="w-48 flex flex-col border border-[#333] bg-panel-bg shrink-0">
                <div className="bg-[#1f1f1f] px-2.5 py-1 text-[11px] font-bold border-b border-[#333] text-gray-300 flex justify-between items-center">
                    <span>COMPRESSOR</span>
                    <div className="w-2 h-2 rounded-full bg-green-500 shadow-[0_0_5px_#00ff00]"></div>
                </div>
                <div className="flex-1 p-3">
                     <div className="w-full h-16 border border-[#222] bg-black relative mb-2">
                        {/* Mock Gain Reduction Meter */}
                        <div className="absolute right-0 top-0 bottom-0 w-2 bg-gradient-to-b from-green-900 to-green-500 opacity-50"></div>
                     </div>
                </div>
            </div>

            <div className="w-48 flex flex-col border border-[#333] bg-panel-bg shrink-0">
                <div className="bg-[#1f1f1f] px-2.5 py-1 text-[11px] font-bold border-b border-[#333] text-gray-300 flex justify-between items-center">
                    <span>EQ-3</span>
                    <div className="w-2 h-2 rounded-full bg-yellow-500"></div>
                </div>
                <div className="flex-1 p-2 flex items-end justify-between gap-1">
                    <div className="w-8 h-20 bg-[#222] relative rounded-sm overflow-hidden">
                        <div className="absolute bottom-0 w-full bg-gray-500 h-[60%]"></div>
                    </div>
                    <div className="w-8 h-20 bg-[#222] relative rounded-sm overflow-hidden">
                        <div className="absolute bottom-0 w-full bg-gray-500 h-[40%]"></div>
                    </div>
                    <div className="w-8 h-20 bg-[#222] relative rounded-sm overflow-hidden">
                        <div className="absolute bottom-0 w-full bg-gray-500 h-[75%]"></div>
                    </div>
                </div>
            </div>

            <button className="w-24 border border-dashed border-[#444] text-[#666] flex flex-col items-center justify-center text-xs hover:text-text-main hover:border-text-muted hover:bg-[#1a1a1a] cursor-pointer transition-all shrink-0 rounded-sm">
                <Plus size={20} className="mb-1" />
                <span>Add FX</span>
            </button>
        </div>
    );
};