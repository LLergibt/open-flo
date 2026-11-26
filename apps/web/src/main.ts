import { mount } from 'svelte';
import App from './App.svelte';
import './style.css';

const target = document.getElementById('app');

if (!target) {
  throw new Error('Missing app mount element');
}

mount(App, { target });

if (import.meta.env.PROD && 'serviceWorker' in navigator) {
  void navigator.serviceWorker.register('/sw.js').catch(() => {
    // Calendar still works online when service workers are unavailable.
  });
}
