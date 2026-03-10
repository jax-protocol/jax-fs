import { Component, createSignal, onMount, onCleanup, For, Show, createEffect } from 'solid-js';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import {
  listLogFiles,
  readLogFile,
  tailLogFile,
  subscribeLogs,
  unsubscribeLogs,
  LogFileInfo,
} from '../lib/api';

type Tab = 'live' | 'files';
type LogLevel = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE';

const ALL_LEVELS: LogLevel[] = ['ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE'];

function detectLevel(line: string): LogLevel | null {
  if (line.includes(' ERROR ') || line.includes('ERROR]')) return 'ERROR';
  if (line.includes(' WARN ') || line.includes('WARN]')) return 'WARN';
  if (line.includes(' INFO ') || line.includes('INFO]')) return 'INFO';
  if (line.includes(' DEBUG ') || line.includes('DEBUG]')) return 'DEBUG';
  if (line.includes(' TRACE ') || line.includes('TRACE]')) return 'TRACE';
  return null;
}

function levelColor(level: LogLevel | null): string {
  switch (level) {
    case 'ERROR': return 'var(--accent-red, #ef4444)';
    case 'WARN': return 'var(--accent-yellow, #f59e0b)';
    case 'INFO': return 'var(--accent-blue, #3b82f6)';
    case 'DEBUG': return 'var(--muted-fg, #888)';
    case 'TRACE': return 'var(--muted-fg, #666)';
    default: return 'var(--fg)';
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const Logs: Component = () => {
  const [tab, setTab] = createSignal<Tab>('live');
  const [liveLines, setLiveLines] = createSignal<string[]>([]);
  const [paused, setPaused] = createSignal(false);
  const [filter, setFilter] = createSignal('');
  const [enabledLevels, setEnabledLevels] = createSignal<Set<LogLevel>>(new Set(ALL_LEVELS));
  const [logFiles, setLogFiles] = createSignal<LogFileInfo[]>([]);
  const [selectedFile, setSelectedFile] = createSignal<string | null>(null);
  const [fileLines, setFileLines] = createSignal<string[]>([]);
  const [fileOffset, setFileOffset] = createSignal(0);
  const [fileTotalLines, setFileTotalLines] = createSignal(0);
  const [fileHasMore, setFileHasMore] = createSignal(false);
  const [fileLoading, setFileLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  let logContainerRef: HTMLDivElement | undefined;
  let unlisten: UnlistenFn | null = null;
  let pausedBuffer: string[] = [];

  const filteredLines = (lines: string[]) => {
    const text = filter().toLowerCase();
    const levels = enabledLevels();
    return lines.filter(line => {
      const level = detectLevel(line);
      if (level && !levels.has(level)) return false;
      if (text && !line.toLowerCase().includes(text)) return false;
      return true;
    });
  };

  const startLiveTail = async () => {
    try {
      // Load initial tail
      const initial = await tailLogFile(500);
      setLiveLines(initial);

      // Subscribe to live updates
      await subscribeLogs();

      unlisten = await listen<string>('log-line', (event) => {
        if (paused()) {
          pausedBuffer.push(event.payload);
          if (pausedBuffer.length > 5000) pausedBuffer.shift();
        } else {
          setLiveLines(prev => {
            const next = [...prev, event.payload];
            if (next.length > 5000) next.splice(0, next.length - 5000);
            return next;
          });
        }
      });
    } catch (e) {
      setError(String(e));
    }
  };

  const stopLiveTail = async () => {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
    try {
      await unsubscribeLogs();
    } catch {
      // ignore
    }
  };

  const togglePause = () => {
    if (paused()) {
      // Resume: flush buffer
      setLiveLines(prev => {
        const next = [...prev, ...pausedBuffer];
        pausedBuffer = [];
        if (next.length > 5000) next.splice(0, next.length - 5000);
        return next;
      });
    }
    setPaused(!paused());
  };

  const toggleLevel = (level: LogLevel) => {
    setEnabledLevels(prev => {
      const next = new Set(prev);
      if (next.has(level)) {
        next.delete(level);
      } else {
        next.add(level);
      }
      return next;
    });
  };

  const loadLogFiles = async () => {
    try {
      const files = await listLogFiles();
      setLogFiles(files);
    } catch (e) {
      setError(String(e));
    }
  };

  const openFile = async (filename: string) => {
    setSelectedFile(filename);
    setFileLoading(true);
    setFileOffset(0);
    try {
      const result = await readLogFile(filename, 0, 1000);
      setFileLines(result.lines);
      setFileTotalLines(result.total_lines);
      setFileHasMore(result.has_more);
      setFileOffset(result.lines.length);
    } catch (e) {
      setError(String(e));
    } finally {
      setFileLoading(false);
    }
  };

  const loadMore = async () => {
    const file = selectedFile();
    if (!file) return;
    setFileLoading(true);
    try {
      const result = await readLogFile(file, fileOffset(), 1000);
      setFileLines(prev => [...prev, ...result.lines]);
      setFileHasMore(result.has_more);
      setFileOffset(prev => prev + result.lines.length);
    } catch (e) {
      setError(String(e));
    } finally {
      setFileLoading(false);
    }
  };

  // Auto-scroll when new lines arrive
  createEffect(() => {
    const _ = liveLines();
    if (!paused() && logContainerRef) {
      requestAnimationFrame(() => {
        if (logContainerRef) {
          logContainerRef.scrollTop = logContainerRef.scrollHeight;
        }
      });
    }
  });

  onMount(() => {
    startLiveTail();
    loadLogFiles();
  });

  onCleanup(() => {
    stopLiveTail();
  });

  return (
    <div style={{ 'max-width': '100%' }}>
      <div style={{ display: 'flex', 'align-items': 'center', 'justify-content': 'space-between', 'margin-bottom': '1rem' }}>
        <h2 style={{ 'font-size': '1.5rem', 'font-weight': '700', margin: '0' }}>Logs</h2>
        <div style={{ display: 'flex', gap: '0.25rem' }}>
          <TabButton label="Live Tail" active={tab() === 'live'} onClick={() => setTab('live')} />
          <TabButton label="Log Files" active={tab() === 'files'} onClick={() => { setTab('files'); loadLogFiles(); }} />
        </div>
      </div>

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
          <button
            onClick={() => setError(null)}
            style={{ float: 'right', background: 'none', border: 'none', color: 'inherit', cursor: 'pointer' }}
          >&times;</button>
        </div>
      </Show>

      {/* Controls */}
      <div style={{
        display: 'flex',
        'align-items': 'center',
        gap: '0.5rem',
        'margin-bottom': '0.75rem',
        'flex-wrap': 'wrap',
      }}>
        <Show when={tab() === 'live'}>
          <button
            onClick={togglePause}
            style={{
              padding: '0.25rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: paused() ? 'var(--accent-yellow, #f59e0b)' : 'var(--bg)',
              color: paused() ? 'white' : 'var(--fg)',
              cursor: 'pointer',
              'font-size': '0.75rem',
              'font-family': 'inherit',
            }}
          >
            {paused() ? 'Resume' : 'Pause'}
          </button>
          <button
            onClick={() => setLiveLines([])}
            style={{
              padding: '0.25rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: 'var(--bg)',
              color: 'var(--fg)',
              cursor: 'pointer',
              'font-size': '0.75rem',
              'font-family': 'inherit',
            }}
          >
            Clear
          </button>
        </Show>

        {/* Level filters */}
        <div style={{ display: 'flex', gap: '0.25rem' }}>
          <For each={ALL_LEVELS}>
            {(level) => (
              <button
                onClick={() => toggleLevel(level)}
                style={{
                  padding: '0.125rem 0.5rem',
                  'border-radius': '4px',
                  border: '1px solid ' + (enabledLevels().has(level) ? levelColor(level) : 'var(--border)'),
                  background: enabledLevels().has(level) ? levelColor(level) + '18' : 'transparent',
                  color: enabledLevels().has(level) ? levelColor(level) : 'var(--muted-fg)',
                  cursor: 'pointer',
                  'font-size': '0.6875rem',
                  'font-family': 'monospace',
                  'font-weight': '600',
                }}
              >
                {level}
              </button>
            )}
          </For>
        </div>

        {/* Text filter */}
        <input
          type="text"
          placeholder="Filter..."
          value={filter()}
          onInput={(e) => setFilter(e.currentTarget.value)}
          style={{
            padding: '0.25rem 0.5rem',
            'border-radius': '6px',
            border: '1px solid var(--border)',
            background: 'var(--bg)',
            color: 'var(--fg)',
            'font-size': '0.75rem',
            'font-family': 'monospace',
            'min-width': '150px',
            flex: '1',
            'max-width': '300px',
            outline: 'none',
          }}
        />
      </div>

      {/* Live tail */}
      <Show when={tab() === 'live'}>
        <div
          ref={logContainerRef}
          style={{
            background: 'var(--bg)',
            border: '1px solid var(--border)',
            'border-radius': '8px',
            padding: '0.5rem',
            height: 'calc(100vh - 220px)',
            overflow: 'auto',
            'font-family': 'monospace',
            'font-size': '0.75rem',
            'line-height': '1.5',
            'white-space': 'pre-wrap',
            'word-break': 'break-all',
          }}
        >
          <Show when={filteredLines(liveLines()).length === 0}>
            <div style={{ color: 'var(--muted-fg)', padding: '1rem', 'text-align': 'center' }}>
              {liveLines().length === 0 ? 'Waiting for log output...' : 'No lines match current filters'}
            </div>
          </Show>
          <For each={filteredLines(liveLines())}>
            {(line) => <LogLine line={line} />}
          </For>
        </div>
        <Show when={paused()}>
          <div style={{
            'text-align': 'center',
            padding: '0.375rem',
            color: 'var(--accent-yellow, #f59e0b)',
            'font-size': '0.75rem',
          }}>
            Paused - {pausedBuffer.length} lines buffered
          </div>
        </Show>
      </Show>

      {/* Log files browser */}
      <Show when={tab() === 'files'}>
        <Show when={!selectedFile()}>
          <div style={{
            background: 'var(--bg)',
            border: '1px solid var(--border)',
            'border-radius': '8px',
            overflow: 'hidden',
          }}>
            <Show when={logFiles().length === 0}>
              <div style={{ color: 'var(--muted-fg)', padding: '1.5rem', 'text-align': 'center', 'font-size': '0.875rem' }}>
                No log files found
              </div>
            </Show>
            <For each={logFiles()}>
              {(file) => (
                <div
                  onClick={() => openFile(file.filename)}
                  style={{
                    display: 'flex',
                    'justify-content': 'space-between',
                    'align-items': 'center',
                    padding: '0.75rem 1rem',
                    cursor: 'pointer',
                    'border-bottom': '1px solid var(--border)',
                    transition: 'background 0.1s ease',
                  }}
                  onMouseEnter={(e) => e.currentTarget.style.background = 'var(--muted)'}
                  onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
                >
                  <div>
                    <div style={{ 'font-size': '0.875rem', 'font-family': 'monospace' }}>{file.filename}</div>
                    <div style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)' }}>
                      {formatSize(file.size)}
                    </div>
                  </div>
                  <span style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>&rsaquo;</span>
                </div>
              )}
            </For>
          </div>
        </Show>

        <Show when={selectedFile()}>
          <div style={{ 'margin-bottom': '0.5rem' }}>
            <button
              onClick={() => { setSelectedFile(null); setFileLines([]); }}
              style={{
                padding: '0.25rem 0.625rem',
                'border-radius': '6px',
                border: '1px solid var(--border)',
                background: 'var(--bg)',
                color: 'var(--fg)',
                cursor: 'pointer',
                'font-size': '0.75rem',
                'font-family': 'inherit',
              }}
            >
              &larr; Back
            </button>
            <span style={{ 'margin-left': '0.5rem', 'font-family': 'monospace', 'font-size': '0.875rem' }}>
              {selectedFile()} ({fileTotalLines()} lines)
            </span>
          </div>

          <div style={{
            background: 'var(--bg)',
            border: '1px solid var(--border)',
            'border-radius': '8px',
            padding: '0.5rem',
            height: 'calc(100vh - 260px)',
            overflow: 'auto',
            'font-family': 'monospace',
            'font-size': '0.75rem',
            'line-height': '1.5',
            'white-space': 'pre-wrap',
            'word-break': 'break-all',
          }}>
            <For each={filteredLines(fileLines())}>
              {(line) => <LogLine line={line} />}
            </For>
            <Show when={fileHasMore() && !fileLoading()}>
              <div style={{ 'text-align': 'center', padding: '0.5rem' }}>
                <button
                  onClick={loadMore}
                  style={{
                    padding: '0.25rem 0.75rem',
                    'border-radius': '6px',
                    border: '1px solid var(--border)',
                    background: 'var(--bg)',
                    color: 'var(--fg)',
                    cursor: 'pointer',
                    'font-size': '0.75rem',
                    'font-family': 'inherit',
                  }}
                >
                  Load more...
                </button>
              </div>
            </Show>
            <Show when={fileLoading()}>
              <div style={{ 'text-align': 'center', padding: '0.5rem', color: 'var(--muted-fg)', 'font-size': '0.75rem' }}>
                Loading...
              </div>
            </Show>
          </div>
        </Show>
      </Show>
    </div>
  );
};

const LogLine: Component<{ line: string }> = (props) => {
  const level = () => detectLevel(props.line);
  return (
    <div style={{
      color: levelColor(level()),
      padding: '0 0.25rem',
      'border-left': level() === 'ERROR' ? '2px solid var(--accent-red, #ef4444)' : level() === 'WARN' ? '2px solid var(--accent-yellow, #f59e0b)' : '2px solid transparent',
    }}>
      {props.line}
    </div>
  );
};

const TabButton: Component<{ label: string; active: boolean; onClick: () => void }> = (props) => (
  <button
    onClick={props.onClick}
    style={{
      padding: '0.375rem 0.75rem',
      'border-radius': '6px',
      border: '1px solid ' + (props.active ? 'var(--fg)' : 'var(--border)'),
      background: props.active ? 'var(--fg)' : 'transparent',
      color: props.active ? 'var(--bg)' : 'var(--fg)',
      cursor: 'pointer',
      'font-size': '0.8125rem',
      'font-weight': props.active ? '600' : '400',
      'font-family': 'inherit',
    }}
  >
    {props.label}
  </button>
);

export default Logs;
