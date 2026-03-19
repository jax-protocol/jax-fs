import { Component, createSignal, onMount, Show } from 'solid-js';
import { getConfigInfo, ConfigInfo } from '../lib/api';

type ThemeOption = 'system' | 'light' | 'dark';
const RELEASES_URL = 'https://github.com/jax-protocol/jax-fs/releases';

const Settings: Component = () => {
  // Auto-launch state
  const [autoLaunch, setAutoLaunch] = createSignal(false);
  const [autoLaunchLoading, setAutoLaunchLoading] = createSignal(true);

  // Theme state
  const [theme, setTheme] = createSignal<ThemeOption>('system');

  // Config info
  const [configInfo, setConfigInfo] = createSignal<ConfigInfo | null>(null);
  const [configLoading, setConfigLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  onMount(async () => {
    // Load auto-launch state
    try {
      const { isEnabled } = await import('@tauri-apps/plugin-autostart');
      const enabled = await isEnabled();
      setAutoLaunch(enabled);
    } catch (_e) {
      // Plugin may not be available
    } finally {
      setAutoLaunchLoading(false);
    }

    // Load theme from localStorage
    const saved = localStorage.getItem('jax-theme') as ThemeOption | null;
    if (saved === 'light' || saved === 'dark') {
      setTheme(saved);
    }

    // Load config info
    try {
      const info = await getConfigInfo();
      setConfigInfo(info);
    } catch (e) {
      setError(String(e));
    } finally {
      setConfigLoading(false);
    }
  });

  const toggleAutoLaunch = async () => {
    try {
      if (autoLaunch()) {
        const { disable } = await import('@tauri-apps/plugin-autostart');
        await disable();
        setAutoLaunch(false);
      } else {
        const { enable } = await import('@tauri-apps/plugin-autostart');
        await enable();
        setAutoLaunch(true);
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const openReleases = () => {
    window.open(RELEASES_URL, '_blank');
  };

  const applyTheme = (value: ThemeOption) => {
    setTheme(value);
    if (value === 'system') {
      localStorage.removeItem('jax-theme');
      document.documentElement.removeAttribute('data-theme');
    } else {
      localStorage.setItem('jax-theme', value);
      document.documentElement.setAttribute('data-theme', value);
    }
  };

  return (
    <div style={{ 'max-width': '640px' }}>
      <h2 style={{ 'font-size': '1.5rem', 'font-weight': '700', 'margin-bottom': '1.5rem' }}>
        Settings
      </h2>

      <Show when={error()}>
        <div style={{
          background: 'hsl(0 84% 60% / 0.08)',
          border: '1px solid hsl(0 84% 60% / 0.3)',
          padding: '0.75rem 1rem',
          'border-radius': '8px',
          'margin-bottom': '1rem',
          color: 'var(--accent-red)',
          'font-size': '0.875rem',
        }}>
          {error()}
        </div>
      </Show>

      {/* General */}
      <div style={cardStyle()}>
        <h3 style={sectionHeaderStyle()}>General</h3>

        {/* Auto-launch toggle */}
        <div style={settingRowStyle()}>
          <div>
            <div style={{ 'font-size': '0.875rem', 'font-weight': '500' }}>Launch at Login</div>
            <div style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)' }}>
              Start Jax automatically when you log in
            </div>
          </div>
          <Show when={!autoLaunchLoading()}>
            <button
              onClick={toggleAutoLaunch}
              style={toggleStyle(autoLaunch())}
            >
              <span style={toggleKnobStyle(autoLaunch())} />
            </button>
          </Show>
        </div>
      </div>

      {/* Updates */}
      <div style={cardStyle()}>
        <h3 style={sectionHeaderStyle()}>Updates</h3>

        <div style={settingRowStyle()}>
          <div>
            <div style={{ 'font-size': '0.875rem', 'font-weight': '500' }}>Check for updates</div>
            <div style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)' }}>
              Download the latest version from GitHub Releases
            </div>
          </div>
          <button
            onClick={openReleases}
            style={{
              padding: '0.375rem 0.75rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: 'var(--bg)',
              color: 'var(--fg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-family': 'inherit',
              'flex-shrink': '0',
            }}
          >
            View Releases
          </button>
        </div>
      </div>

      {/* Appearance */}
      <div style={cardStyle()}>
        <h3 style={sectionHeaderStyle()}>Appearance</h3>

        <div style={{
          display: 'flex',
          gap: '0.5rem',
        }}>
          {(['system', 'light', 'dark'] as ThemeOption[]).map(opt => (
            <button
              onClick={() => applyTheme(opt)}
              style={{
                flex: '1',
                padding: '0.5rem 0.75rem',
                'border-radius': '8px',
                border: '1px solid ' + (theme() === opt ? 'var(--fg)' : 'var(--border)'),
                background: theme() === opt ? 'var(--fg)' : 'var(--bg)',
                color: theme() === opt ? 'var(--bg)' : 'var(--fg)',
                cursor: 'pointer',
                'font-size': '0.8125rem',
                'font-weight': theme() === opt ? '600' : '400',
                'font-family': 'inherit',
                'text-transform': 'capitalize',
              }}
            >
              {opt}
            </button>
          ))}
        </div>
      </div>

      {/* Local Configuration */}
      <div style={cardStyle()}>
        <h3 style={sectionHeaderStyle()}>Local Configuration</h3>

        <Show when={configLoading()}>
          <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>Loading...</p>
        </Show>

        <Show when={configInfo()}>
          <div style={{ display: 'flex', 'flex-direction': 'column', gap: '0.75rem' }}>
            <ConfigRow label="Data directory" value={configInfo()!.jax_dir} />
            <ConfigRow label="Database" value={configInfo()!.db_path} />
            <ConfigRow label="Config file" value={configInfo()!.config_path} />
            <ConfigRow label="Blob store" value={configInfo()!.blob_store} />
          </div>
        </Show>
      </div>
    </div>
  );
};

const ConfigRow: Component<{ label: string; value: string }> = (props) => (
  <div>
    <div style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)', 'margin-bottom': '0.125rem' }}>
      {props.label}
    </div>
    <div style={{
      'font-size': '0.75rem',
      'font-family': 'monospace',
      'word-break': 'break-all',
      background: 'var(--bg)',
      border: '1px solid var(--border)',
      'border-radius': '6px',
      padding: '0.375rem 0.625rem',
    }}>
      {props.value}
    </div>
  </div>
);

function cardStyle(): Record<string, string> {
  return {
    background: 'var(--muted)',
    border: '1px solid var(--border)',
    'border-radius': 'var(--radius)',
    padding: '1.5rem',
    'margin-bottom': '1rem',
  };
}

function sectionHeaderStyle(): Record<string, string> {
  return {
    'font-size': '0.75rem',
    'font-weight': '600',
    'text-transform': 'uppercase',
    'letter-spacing': '0.05em',
    color: 'var(--muted-fg)',
    'margin-bottom': '1rem',
  };
}

function settingRowStyle(): Record<string, string> {
  return {
    display: 'flex',
    'justify-content': 'space-between',
    'align-items': 'center',
  };
}

function toggleStyle(on: boolean): Record<string, string> {
  return {
    width: '44px',
    height: '24px',
    'border-radius': '12px',
    border: 'none',
    background: on ? 'var(--accent-green)' : 'var(--border)',
    cursor: 'pointer',
    position: 'relative',
    transition: 'background 0.2s ease',
    'flex-shrink': '0',
  };
}

function toggleKnobStyle(on: boolean): Record<string, string> {
  return {
    position: 'absolute',
    top: '2px',
    left: on ? '22px' : '2px',
    width: '20px',
    height: '20px',
    'border-radius': '50%',
    background: 'white',
    transition: 'left 0.2s ease',
    'box-shadow': '0 1px 3px rgba(0,0,0,0.2)',
  };
}

export default Settings;
