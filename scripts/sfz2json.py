#!/usr/bin/env python3
"""
sfz2json.py  —  SFZ → OSMP-parser JSON

Converts a real-world .sfz instrument (with deep #include chains, $define
variables, <global>/<master>/<group>/<region> hierarchy and CC modulations)
into a flat JSON file optimised for the OSMP sampler engine.

Kontakt-style output schema
───────────────────────────
{
  "name":       "<sfz stem>",
  "cc_labels":  { "70": "Kick mic", ... },
  "cc_init":    { "70": 100, ... },
  "defines":    { "$kickkey": "36", ... },
  "zones": [
    {
      "sample":        "<path relative to sfz>",
      "lo_key":        <int>,
      "hi_key":        <int>,
      "root_key":      <int>,
      "lo_vel":        <int 0-127>,
      "hi_vel":        <int 0-127>,
      "loop_mode":     "one_shot" | "no_loop" | "loop_continuous" | ...,
      "seq_length":    <int>,
      "seq_position":  <int>,
      "trigger":       "attack" | "release" | "first" | "legato",
      "group_id":      <int>,
      "off_by":        <int>,
      "off_mode":      "fast" | "normal",
      "tune_cents":    <int>,
      "transpose":     <int semitones>,
      "volume_db":     <float>,
      "amplitude":     <float 0-100>,
      "pan":           <float -100..100>,
      "amp_velcurve":  { "<vel>": <float>, ... },
      "ampeg": {
        "attack": <f>, "hold": <f>, "decay": <f>,
        "sustain": <f 0-100>, "release": <f>
      },
      "cc_conditions": { "<n>": [lo, hi], ... },
      "cc_mods":       { "amplitude_cc70": 100, ... }
    }
  ]
}
"""

import json
import re
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# ── Note name helpers ──────────────────────────────────────────────────────────

_NOTE = {
    'c': 0, 'c#': 1, 'db': 1, 'd': 2, 'd#': 3, 'eb': 3,
    'e': 4, 'f': 5, 'f#': 6, 'gb': 6, 'g': 7, 'g#': 8,
    'ab': 8, 'a': 9, 'a#': 10, 'bb': 10, 'b': 11,
}

def note_to_midi(s: str) -> Optional[int]:
    s = s.strip().lower()
    if re.fullmatch(r'-?\d+', s):
        return int(s)
    m = re.fullmatch(r'([a-g][#b]?)(-?\d+)', s)
    if m:
        return _NOTE[m.group(1)] + (int(m.group(2)) + 1) * 12
    return None

# ── Opcode parser ──────────────────────────────────────────────────────────────

def parse_opcodes(text: str) -> Dict[str, str]:
    """Parse 'key=value key2=value2 ...' pairs from raw text.

    Values can contain spaces (sample paths).  Value extends from after '='
    up to the next 'word=' boundary or end of string.
    """
    result: Dict[str, str] = {}
    # match opcode= positions; value is everything until next match
    matches = list(re.finditer(r'\b([A-Za-z_]\w*)\s*=\s*', text))
    for i, m in enumerate(matches):
        key = m.group(1).lower()
        v_start = m.end()
        v_end   = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        val = text[v_start:v_end].strip()
        if val:
            result[key] = val
    return result

# ── SFZ file loader (handles #define, #include, comments) ─────────────────────

class SfzLoader:
    def __init__(self, root: Path, extra_define_files: List[Path] = None):
        self.defines: Dict[str, str] = {}
        self._visited: set = set()
        self._root: Path = root.resolve()

        # Pre-load any external keymap / define files
        if extra_define_files:
            for p in extra_define_files:
                if p.exists():
                    self._collect_defines(p.read_text('utf-8', errors='replace'))

    # ── pre-pass: collect all #defines ────────────────────────────────────────

    def _collect_defines(self, text: str) -> None:
        text = self._strip_comments(text)
        for m in re.finditer(r'#define\s+(\$\w+)\s+(\S+)', text):
            self.defines[m.group(1)] = m.group(2)

    # ── main load: returns flat text with includes expanded ───────────────────

    def load(self, path: Path) -> str:
        path = path.resolve()
        if path in self._visited:
            return ''
        self._visited.add(path)

        try:
            raw = path.read_text('utf-8', errors='replace')
        except FileNotFoundError:
            print(f'  [warn] include not found: {path}', file=sys.stderr)
            return ''

        # Collect defines from this file first (so they're available for subs)
        self._collect_defines(raw)
        raw = self._strip_comments(raw)
        raw = self._remove_define_lines(raw)

        # Expand #include directives
        def expand_include(m: re.Match) -> str:
            inc_name = m.group(1).strip().strip('"\'')
            # Many SFZ kits write paths relative to the root SFZ directory even
            # from deep sub-files.  Try root-relative first, then file-relative.
            root_rel = self._root / inc_name
            file_rel = path.parent / inc_name
            inc_path = root_rel if root_rel.exists() else file_rel
            return '\n' + self.load(inc_path) + '\n'

        raw = re.sub(r'#include\s+["\']([^"\']+)["\']', expand_include, raw)

        # Substitute $VARIABLES
        for var, val in sorted(self.defines.items(), key=lambda x: -len(x[0])):
            raw = raw.replace(var, val)

        return raw

    @staticmethod
    def _strip_comments(text: str) -> str:
        text = re.sub(r'/\*.*?\*/', ' ', text, flags=re.DOTALL)
        text = re.sub(r'//[^\n]*', '', text)
        return text

    @staticmethod
    def _remove_define_lines(text: str) -> str:
        return re.sub(r'#define\s+\$\w+\s+\S+', '', text)

# ── SFZ section tokeniser ──────────────────────────────────────────────────────

SECTION_HEADERS = {'control', 'global', 'master', 'group', 'region',
                   'effect', 'curve', 'midi', 'sample'}

def tokenise_sections(flat_text: str) -> List[Dict]:
    """Split flat SFZ text into a list of {'header': str, 'body': str}."""
    sections: List[Dict] = []
    # Split on <header> tags
    parts = re.split(r'(<\w+>)', flat_text)
    current_header = None
    current_body   = ''

    for part in parts:
        hm = re.fullmatch(r'<(\w+)>', part.strip())
        if hm:
            if current_header is not None:
                sections.append({'header': current_header.lower(),
                                  'body':   current_body})
            current_header = hm.group(1).lower()
            current_body   = ''
        else:
            current_body += part

    if current_header:
        sections.append({'header': current_header, 'body': current_body})

    # Parse opcodes for each section
    for sec in sections:
        sec['opcodes'] = parse_opcodes(sec['body'])
        del sec['body']

    return sections

# ── Zone builder ───────────────────────────────────────────────────────────────

def _int(d: Dict, key: str, default: int = 0) -> int:
    v = d.get(key)
    if v is None:
        return default
    n = note_to_midi(str(v))
    return n if n is not None else default

def _float(d: Dict, key: str, default: float = 0.0) -> float:
    try:
        return float(d.get(key, default))
    except (ValueError, TypeError):
        return float(default)

def _str(d: Dict, key: str, default: str = '') -> str:
    return str(d.get(key, default))

# CC opcode patterns
_RE_CC_COND  = re.compile(r'^(lo|hi)cc(\d+)$')
_RE_CC_MOD   = re.compile(r'^(.+?)_(on)?cc(\d+)$')
_RE_VELCURVE = re.compile(r'^amp_velcurve_(\d+)$')

def _parse_num(v: str):
    """Parse a numeric SFZ opcode value robustly.

    Strips trailing garbage (extra dots, spaces) that some SFZ files emit.
    Returns int when the value has no fractional part, float otherwise.
    Returns 0 if completely unparseable.
    """
    v = v.strip()
    # Remove trailing non-digit characters (e.g. a stray second '.')
    v = re.sub(r'[^\d\-\.eE]+$', '', v)
    # Collapse multiple decimal points: keep only the first
    parts = v.split('.')
    if len(parts) > 2:
        v = parts[0] + '.' + ''.join(parts[1:])
    if not v or v in ('-', '.'):
        return 0
    try:
        f = float(v)
        return int(f) if f == int(f) and '.' not in v else f
    except ValueError:
        return 0

def build_zone(merged: Dict[str, str]) -> Optional[Dict[str, Any]]:
    """Convert a fully-merged opcode dict into a zone dict.
    Returns None if no sample is present.
    """
    sample = _str(merged, 'sample')
    if not sample:
        return None

    # key= shorthand sets lokey/hikey/pitch_keycenter
    if 'key' in merged:
        kn = note_to_midi(merged['key'])
        if kn is not None:
            merged.setdefault('lokey', str(kn))
            merged.setdefault('hikey', str(kn))
            merged.setdefault('pitch_keycenter', str(kn))

    loop_mode   = _str(merged, 'loop_mode', 'no_loop')
    loop_enabled = loop_mode not in ('no_loop', 'one_shot', '')

    cc_conditions: Dict[str, list]  = {}
    cc_mods:       Dict[str, Any]   = {}
    amp_velcurve:  Dict[str, float] = {}
    extras:        Dict[str, str]   = {}

    # Categorise every remaining opcode
    for k, v in merged.items():
        m_cond = _RE_CC_COND.fullmatch(k)
        if m_cond:
            side, n = m_cond.group(1), m_cond.group(2)
            bucket = cc_conditions.setdefault(n, [0, 127])
            if side == 'lo':
                bucket[0] = int(_parse_num(v))
            else:
                bucket[1] = int(_parse_num(v))
            continue

        m_mod = _RE_CC_MOD.fullmatch(k)
        if m_mod:
            cc_mods[k] = _parse_num(v)
            continue

        m_vc = _RE_VELCURVE.fullmatch(k)
        if m_vc:
            try:
                amp_velcurve[m_vc.group(1)] = float(v)
            except ValueError:
                pass
            continue

    zone: Dict[str, Any] = {
        'sample':       sample.replace('\\', '/'),
        'lo_key':       _int(merged, 'lokey', 0),
        'hi_key':       _int(merged, 'hikey', 127),
        'root_key':     _int(merged, 'pitch_keycenter', 60),
        'lo_vel':       _int(merged, 'lovel', 0),
        'hi_vel':       _int(merged, 'hivel', 127),
        'loop_mode':    loop_mode,
        'loop_start':   _int(merged, 'loop_start'),
        'loop_end':     _int(merged, 'loop_end'),
        'seq_length':   _int(merged, 'seq_length', 1),
        'seq_position': _int(merged, 'seq_position', 1),
        'trigger':      _str(merged, 'trigger', 'attack'),
        'group_id':     _int(merged, 'group'),
        'off_by':       _int(merged, 'off_by'),
        'off_mode':     _str(merged, 'off_mode', 'fast'),
        'tune_cents':   _int(merged, 'tune'),
        'transpose':    _int(merged, 'transpose'),
        'volume_db':    _float(merged, 'volume'),
        'amplitude':    _float(merged, 'amplitude', 100.0),
        'pan':          _float(merged, 'pan'),
        'ampeg': {
            'attack':  _float(merged, 'ampeg_attack',  0.001),
            'hold':    _float(merged, 'ampeg_hold',    0.0),
            'decay':   _float(merged, 'ampeg_decay',   0.0),
            'sustain': _float(merged, 'ampeg_sustain', 100.0),
            'release': _float(merged, 'ampeg_release', 0.05),
        },
    }

    if cc_conditions:
        zone['cc_conditions'] = cc_conditions
    if cc_mods:
        zone['cc_mods'] = cc_mods
    if amp_velcurve:
        zone['amp_velcurve'] = amp_velcurve

    return zone

# ── Main converter ─────────────────────────────────────────────────────────────

def sfz_to_dict(
    sfz_path:   Path,
    extra_defs: List[Path] = None,
) -> Dict[str, Any]:

    loader   = SfzLoader(sfz_path.parent, extra_define_files=extra_defs or [])
    flat     = loader.load(sfz_path)
    sections = tokenise_sections(flat)

    # ── Collect control metadata ───────────────────────────────────────────────
    cc_labels: Dict[str, str]   = {}
    cc_init:   Dict[str, int]   = {}
    global_ops: Dict[str, str]  = {}

    for sec in sections:
        ops = sec['opcodes']
        hdr = sec['header']

        if hdr == 'control':
            for k, v in ops.items():
                m = re.fullmatch(r'label_cc(\d+)', k)
                if m:
                    cc_labels[m.group(1)] = v
                    continue
                m = re.fullmatch(r'set_cc(\d+)', k)
                if m:
                    try:
                        cc_init[m.group(1)] = int(v)
                    except ValueError:
                        pass

        elif hdr in ('global', 'master'):
            if hdr == 'global':
                global_ops.update(ops)

    # ── Walk sections and produce zones ───────────────────────────────────────
    zones: List[Dict] = []

    master_ops: Dict[str, str] = {}
    group_ops:  Dict[str, str] = {}

    for sec in sections:
        hdr = sec['header']
        ops = sec['opcodes']

        if hdr == 'global':
            global_ops = ops

        elif hdr == 'master':
            master_ops = dict(ops)
            group_ops  = {}     # reset group when master changes

        elif hdr == 'group':
            group_ops = dict(ops)

        elif hdr == 'region':
            # Merge: global < master < group < region
            merged = {**global_ops, **master_ops, **group_ops, **ops}
            zone   = build_zone(merged)
            if zone:
                zones.append(zone)

    # ── Assemble output ────────────────────────────────────────────────────────
    return {
        'name':       sfz_path.stem,
        'cc_labels':  cc_labels,
        'cc_init':    cc_init,
        'defines':    loader.defines,
        'zones':      zones,
    }

# ── CLI ────────────────────────────────────────────────────────────────────────

def main() -> None:
    import argparse

    ap = argparse.ArgumentParser(
        description='Convert .sfz to OSMP-parser JSON (Kontakt-style zone mapping)'
    )
    ap.add_argument('sfz',
                    help='Input .sfz file')
    ap.add_argument('-o', '--out',
                    help='Output .json file [default: <sfz>.json]')
    ap.add_argument('-d', '--defines', nargs='*', default=[],
                    metavar='FILE',
                    help='Extra .sfz files to pre-load for $define variables '
                         '(e.g. keymap.sfz)')
    ap.add_argument('--indent', type=int, default=2,
                    help='JSON indent level [default: 2]')
    ap.add_argument('--no-defines', action='store_true',
                    help='Omit the defines dict from output')
    args = ap.parse_args()

    sfz_path  = Path(args.sfz)
    out_path  = Path(args.out) if args.out else sfz_path.with_suffix('.json')
    extra     = [Path(d) for d in args.defines]

    print(f'parsing  {sfz_path}', file=sys.stderr)
    data = sfz_to_dict(sfz_path, extra_defs=extra)

    if args.no_defines:
        del data['defines']

    out_path.write_text(
        json.dumps(data, indent=args.indent, ensure_ascii=False),
        encoding='utf-8'
    )

    n_zones  = len(data['zones'])
    n_labels = len(data.get('cc_labels', {}))
    n_defs   = len(data.get('defines', {}))
    print(
        f'wrote    {out_path}\n'
        f'         {n_zones} zones  |  {n_labels} CC labels  |  {n_defs} $defines',
        file=sys.stderr
    )

if __name__ == '__main__':
    main()
