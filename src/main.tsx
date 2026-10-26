import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './i18n'
import App from './App'
import { installGlobalErrorHandlers } from './lib/logging'
import './index.css'
import { applyNativeMaterial, detectPlatform } from './lib/nativeWindow'

// Platform is known synchronously (title-bar layout), the material is async.
document.documentElement.dataset.platform = detectPlatform();
void applyNativeMaterial();

// Before React mounts: an error thrown during the first render must reach the
// log file too, and in a release build the WebView console cannot be opened.
installGlobalErrorHandlers();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
