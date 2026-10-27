import React from 'react';
import { createRoot, hydrateRoot } from 'react-dom/client';
import { BrowserRouter, useRoutes } from 'react-router-dom';
import { HelmetProvider } from 'react-helmet-async';
import { createAppRoutes } from './routes';
import './styles/index.css';

const AppRoutes = () => useRoutes(createAppRoutes());

const tree = (
  <React.StrictMode>
    <HelmetProvider>
      <BrowserRouter>
        <AppRoutes />
      </BrowserRouter>
    </HelmetProvider>
  </React.StrictMode>
);

const container = document.getElementById('root')!;

/**
 * Prerendered pages arrive with markup already inside `#root`; hydrating keeps
 * it and only attaches event handlers, so the first paint is the static HTML.
 * In `vite dev` the container is empty, so there is nothing to hydrate.
 *
 * `BrowserRouter` rather than `createBrowserRouter`: the prerenderer renders
 * through `StaticRouter`, and the declarative router is the pairing that
 * produces matching markup. A data router keeps internal state the static pass
 * cannot reproduce, which React would report as a hydration mismatch.
 */
if (container.hasChildNodes()) {
  hydrateRoot(container, tree);
} else {
  createRoot(container).render(tree);
}
