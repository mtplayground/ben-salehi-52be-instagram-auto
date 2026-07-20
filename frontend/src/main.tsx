import React from 'react';
import ReactDOM from 'react-dom/client';
import { Navigate, RouterProvider, createBrowserRouter } from 'react-router-dom';

import { AppLayout } from './app/AppLayout';
import { AuthGate } from './auth/AuthGate';
import { AuthProvider } from './auth/AuthProvider';
import { OverviewPage } from './pages/OverviewPage';
import { QueuePage } from './pages/QueuePage';
import { SettingsPage } from './pages/SettingsPage';
import './styles.css';

const router = createBrowserRouter([
  {
    path: '/',
    element: <AuthGate />,
    children: [
      {
        element: <AppLayout />,
        children: [
          { index: true, element: <OverviewPage /> },
          { path: 'queue', element: <QueuePage /> },
          { path: 'settings', element: <SettingsPage /> },
        ],
      },
      { path: '*', element: <Navigate to="/" replace /> },
    ],
  },
]);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <AuthProvider>
      <RouterProvider router={router} />
    </AuthProvider>
  </React.StrictMode>,
);
