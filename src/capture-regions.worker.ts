import { detectRegions } from './geometry'

// Only receives a bounded, local pixel probe. No screen or network access.
self.onmessage = (event: MessageEvent<{ pixels: Uint8ClampedArray; width: number; height: number }>) => {
  const { pixels, width, height } = event.data
  self.postMessage(detectRegions(pixels, width, height))
}
