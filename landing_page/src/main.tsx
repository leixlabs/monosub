import React from 'react';
import ReactDOM from 'react-dom/client';
import { createBrowserRouter, RouterProvider, Navigate, redirect } from 'react-router-dom';
import App from './App';
import { detectBrowserLanguage, DEFAULT_LANGUAGE, translations } from './i18n';
import './styles/index.css';

const validLangs = Object.keys(translations);

/** Old hreflang URLs (`/zh-tw/`, `/ja/`, …) are still indexed; send them home. */
const resolveLanguage = (lang: string): string => {
  const lower = lang.toLowerCase();
  if (lower === 'zh' || lower.startsWith('zh-')) return 'zh';
  return DEFAULT_LANGUAGE;
};

const router = createBrowserRouter([
  {
    path: '/',
    element: <Navigate to={`/${detectBrowserLanguage()}`} replace />
  },
  {
    path: '/:lang',
    element: <App />,
    loader: ({ params }) => {
      const lang = params.lang!;
      if (!validLangs.includes(lang)) {
        throw redirect(`/${resolveLanguage(lang)}`);
      }
      return null;
    }
  },
  {
    path: '*',
    loader: () => {
      throw redirect(`/${detectBrowserLanguage()}`);
    }
  }
]);

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <RouterProvider router={router} />
  </React.StrictMode>
);
