export interface OsmpZoneSlot {
  note: number;
  label: string;
  sample: string;
  lo_key: number;
  hi_key: number;
  root_key: number;
  lo_vel: number;
  hi_vel: number;
  group_id: number;
  zone_count: number;
  seq_length: number;
  ampeg_attack: number;
  ampeg_hold: number;
  ampeg_decay: number;
  ampeg_sustain: number;
  ampeg_release: number;
  volume_db: number;
  pan: number;
}

export interface OsmpZonesData {
  name: string;
  cc_labels: Record<string, string>;
  cc_init: Record<string, number>;
  cc_state: [number, number][];
  pad_slots: OsmpZoneSlot[];
}
