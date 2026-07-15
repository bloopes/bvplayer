// src/types/index.ts

/**
 * 单首歌曲数据结构
 */
export interface Song {
  bvid: string;
  cid: number;
  title: string;
  author: string;
  cover_url: string;
  audio_url: string;
  duration: number;
}

/**
 * 物理音频设备数据结构
 */
export interface AudioDevice {
  name: string;
  is_default: boolean;
}

/**
 * B站链接导入的请求体结构
 */
export interface ImportPayload {
  bvid?: string;
  sid?: string;
  fid?: string;
  [key: string]: any;
}