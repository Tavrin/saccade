import type { Expect, TestType, Page, Locator } from '@playwright/test';
export interface SaccadeOptions {
  binary?: string; metric?: 'mean' | 'p95' | 'p99' | 'max'; threshold?: number;
  align?: 'none' | 'translation' | 'similarity' | 'affine' | 'homography' | 'auto'; resample?: 'reference' | 'common';
  profile?: string; projectConfig?: string; config?: string;
  masks?: Array<{ reason: string; selector?: string; rect?: [number, number, number, number] }>;
  fullPage?: boolean; clock?: string; randomSeed?: number; scrollLazyLoad?: boolean;
  maxScrollSteps?: number; scrollDelayMs?: number; timeout?: number; cliTimeout?: number;
  stabilityCheck?: { delayMs?: number }; updateSnapshots?: boolean;
}
export function install(expect: Expect, test: TestType<any, any>): void;
declare global { namespace PlaywrightTest { interface Matchers<R, T = Page | Locator> { toMatchSaccade(name: string, options?: SaccadeOptions): Promise<R>; } } }
