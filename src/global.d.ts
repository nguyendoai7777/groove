import { DefineComponent, Component } from 'vue';
import { OverlayScrollbars, PartialOptions, EventListeners, OnUpdatedEventListenerArgs } from 'overlayscrollbars';

declare global {
  export type CastString<T> = T | (string & {});
}

declare module 'overlayscrollbars-vue' {
  export interface OverlayScrollbarsComponentProps {
    element?: string | Component;
    options?: PartialOptions | false | null;
    events?: EventListeners | false | null;
    defer?: boolean | IdleRequestOptions;
  }

  export interface OverlayScrollbarsComponentRef {
    osInstance(): OverlayScrollbars | null;
    getElement(): HTMLElement | null;
  }

  export const OverlayScrollbarsComponent: DefineComponent<
    OverlayScrollbarsComponentProps,
    OverlayScrollbarsComponentRef,
    any,
    {},
    {},
    any,
    any,
    {
      osInitialized: (instance: OverlayScrollbars) => void;
      osUpdated: (instance: OverlayScrollbars, info: OnUpdatedEventListenerArgs) => void;
      osDestroyed: (instance: OverlayScrollbars) => void;
      osScroll: (instance: OverlayScrollbars, event: Event) => void;
      'os-initialized': (instance: OverlayScrollbars) => void;
      'os-updated': (instance: OverlayScrollbars, info: OnUpdatedEventListenerArgs) => void;
      'os-destroyed': (instance: OverlayScrollbars) => void;
      'os-scroll': (instance: OverlayScrollbars, event: Event) => void;
    }
  >;

  export function useOverlayScrollbars(params?: any): [any, any];
}

declare module 'vue' {
  export interface GlobalComponents {
    OverlayScrollbarsComponent: typeof import('overlayscrollbars-vue').OverlayScrollbarsComponent;
    'overlay-scrollbars-component': typeof import('overlayscrollbars-vue').OverlayScrollbarsComponent;
  }
}
