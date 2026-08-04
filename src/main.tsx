import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles/globals.css';
// Content-page redesign. Its `st-*` selectors are isolated from the app shell,
// projects and graph, so navigation chrome keeps the existing Nexus language.
import './styles/strata.css';
import './styles/tiptap.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
