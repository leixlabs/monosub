import type { Post } from './types';
import { macwhisper } from './macwhisper';
import { videocaptioner } from './videocaptioner';
import { cloudServices } from './cloud-transcription';
import { burnInWorkflow } from './subtitle-to-video-workflow';
import { pickingAsrModel } from './choosing-an-asr-model';

/**
 * The registry. `scripts/prerender.mjs` reads this list to render pages, build the
 * sitemap and emit JSON-LD, so adding a post means creating the file and adding it
 * here — nothing else.
 *
 * Order matters only as a tie-break: the index sorts by `updated` descending and
 * falls back to this order for posts that share a date.
 */
export const POSTS: Post[] = [
  videocaptioner,
  macwhisper,
  cloudServices,
  burnInWorkflow,
  pickingAsrModel,
];

export const postBySlug = (slug: string): Post | undefined =>
  POSTS.find((post) => post.slug === slug);
