import { Router, Route, A, useLocation } from '@solidjs/router';
import { Component } from 'solid-js';
import Home from './pages/Home';
import Buckets from './pages/Buckets';
import Explorer from './pages/Explorer';
import Viewer from './pages/Viewer';
import Editor from './pages/Editor';
import History from './pages/History';
import Mounts from './pages/Mounts';
import Logs from './pages/Logs';
import Settings from './pages/Settings';

const Layout: Component<{ children?: any }> = (props) => {
  const location = useLocation();

  const navLink = (href: string, label: string, icon: string) => {
    const active = () => location.pathname === href || location.pathname.startsWith(href + '/');
    // Exact match for Home
    const isActive = () => href === '/' ? location.pathname === '/' : active();
    return (
      <A
        href={href}
        style={{
          color: isActive() ? 'var(--fg)' : 'var(--muted-fg)',
          'text-decoration': 'none',
          padding: '0.625rem 0.75rem',
          'border-radius': '8px',
          background: isActive() ? 'var(--muted)' : 'transparent',
          display: 'flex',
          'align-items': 'center',
          gap: '0.5rem',
          'font-size': '0.875rem',
          'font-weight': isActive() ? '600' : '400',
          transition: 'all 0.15s ease',
        }}
      >
        <span style={{ 'font-size': '1rem' }}>{icon}</span>
        {label}
      </A>
    );
  };

  return (
    <div style={{ display: 'flex', 'min-height': '100vh' }}>
      <nav style={{
        width: '220px',
        'border-right': '1px solid var(--border)',
        padding: '1.25rem 0.75rem',
        display: 'flex',
        'flex-direction': 'column',
        gap: '0.25rem',
        'flex-shrink': '0',
      }}>
        <div style={{
          display: 'flex',
          'align-items': 'center',
          gap: '0.5rem',
          padding: '0.5rem 0.75rem',
          'margin-bottom': '1.5rem',
        }}>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <ellipse cx="12" cy="5" rx="9" ry="3"/>
            <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
            <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
          </svg>
          <span style={{ 'font-size': '1.125rem', 'font-weight': '700' }}>jax</span>
        </div>

        {navLink('/', 'Home', '\u2302')}
        {navLink('/buckets', 'Buckets', '\u2750')}
        {navLink('/mounts', 'Mounts', '\u26C1')}
        {navLink('/logs', 'Logs', '\u2261')}

        <div style={{ 'margin-top': 'auto' }}>
          {navLink('/settings', 'Settings', '\u2699')}
        </div>
      </nav>

      <main style={{ flex: 1, padding: '2rem', overflow: 'auto' }}>
        {props.children}
      </main>
    </div>
  );
};

const App: Component = () => {
  return (
    <Router root={Layout}>
      <Route path="/" component={Home} />
      <Route path="/buckets" component={Buckets} />
      <Route path="/buckets/:bucketId" component={Explorer} />
      <Route path="/buckets/:bucketId/view" component={Viewer} />
      <Route path="/buckets/:bucketId/edit" component={Editor} />
      <Route path="/buckets/:bucketId/history" component={History} />
      <Route path="/mounts" component={Mounts} />
      <Route path="/logs" component={Logs} />
      <Route path="/settings" component={Settings} />
    </Router>
  );
};

export default App;
