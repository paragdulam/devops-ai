// @novnc/novnc ships no type declarations; this covers only what this app uses.
declare module "@novnc/novnc" {
  interface RFBOptions {
    credentials?: { username?: string; password?: string; target?: string };
    shared?: boolean;
    wsProtocols?: string[];
  }

  interface RFBCredentials {
    username?: string;
    password?: string;
    target?: string;
  }

  export default class RFB extends EventTarget {
    constructor(target: HTMLElement, urlOrChannel: string, options?: RFBOptions);
    scaleViewport: boolean;
    disconnect(): void;
    sendCredentials(credentials: RFBCredentials): void;
  }
}
