import React from 'react';
import ReactDOM from 'react-dom/client';
import { createBrowserRouter, RouterProvider, Navigate } from 'react-router-dom';
import App from './App';
import { detectBrowserLanguage, translations } from './i18n';
import './styles/index.css';

const validLangs = Object.keys(translations);

const router = createBrowserRouter([
  {
    path: '/',
    element: <Navigate to={`/${detectBrowserLanguage()}`} replace />
  },
  {
    path: '/:lang',
    element: <App />,
    loader: ({ params }) => {
      if (!validLangs.includes(params.lang!)) {
        throw new Response('Not Found', { status: 404 });
      }
      return null;
    }
  }
]);

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <RouterProvider router={router} />
  </React.StrictMode>
);
