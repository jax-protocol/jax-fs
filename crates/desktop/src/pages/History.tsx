import { Component, createSignal, onMount, For, Show } from 'solid-js';
import { useParams, useNavigate } from '@solidjs/router';
import { getHistory, HistoryEntry } from '../lib/api';

const History: Component = () => {
  const params = useParams<{ bucketId: string }>();
  const navigate = useNavigate();

  const [entries, setEntries] = createSignal<HistoryEntry[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [page, setPage] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(true);

  const fetchHistory = async (p: number) => {
    try {
      setLoading(true);
      setError(null);
      const result = await getHistory(params.bucketId, p);
      setEntries(result);
      setHasMore(result.length >= 50);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(() => {
    fetchHistory(0);
  });

  const goToVersion = (entry: HistoryEntry) => {
    navigate(`/buckets/${params.bucketId}?path=/&at=${encodeURIComponent(entry.link_hash)}`);
  };

  const goBack = () => {
    navigate(`/buckets/${params.bucketId}?path=/`);
  };

  const loadPage = (p: number) => {
    setPage(p);
    fetchHistory(p);
  };

  return (
    <div style={{ 'max-width': '720px' }}>
      {/* Header */}
      <div style={{
        display: 'flex',
        'justify-content': 'space-between',
        'align-items': 'center',
        'margin-bottom': '1.5rem',
      }}>
        <div>
          <h2 style={{ 'font-size': '1.5rem', 'font-weight': '700', 'margin-bottom': '0.25rem' }}>
            Version History
          </h2>
          <div style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)', 'font-family': 'monospace' }}>
            {params.bucketId.substring(0, 8)}...
          </div>
        </div>
        <button
          onClick={goBack}
          style={{
            padding: '0.5rem 0.75rem',
            'border-radius': '8px',
            border: '1px solid var(--border)',
            background: 'var(--muted)',
            color: 'var(--fg)',
            cursor: 'pointer',
            'font-size': '0.8125rem',
            'font-family': 'inherit',
          }}
        >
          <span style={{ display: 'inline-flex', 'align-items': 'center', gap: '0.375rem' }}><svg width="14" height="14" viewBox="0 0 16 16" fill="none" style={{ display: 'block' }}><path d="M10 3L5 8L10 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>Back</span>
        </button>
      </div>

      {/* Error */}
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

      {/* Loading */}
      <Show when={loading()}>
        <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>Loading history...</p>
      </Show>

      {/* Empty */}
      <Show when={!loading() && entries().length === 0 && !error()}>
        <div style={{
          background: 'var(--muted)',
          border: '1px solid var(--border)',
          'border-radius': 'var(--radius)',
          padding: '3rem',
          'text-align': 'center',
        }}>
          <p style={{ 'font-size': '0.9375rem', 'font-weight': '500' }}>No history</p>
          <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>This bucket has no recorded versions</p>
        </div>
      </Show>

      {/* Version list */}
      <Show when={!loading() && entries().length > 0}>
        <div style={{
          border: '1px solid var(--border)',
          'border-radius': 'var(--radius)',
          overflow: 'hidden',
        }}>
          {/* Header */}
          <div style={{
            display: 'grid',
            'grid-template-columns': '60px 1fr 100px 80px',
            padding: '0.625rem 1rem',
            background: 'var(--muted)',
            'border-bottom': '1px solid var(--border)',
            'font-size': '0.75rem',
            'font-weight': '600',
            'text-transform': 'uppercase',
            'letter-spacing': '0.05em',
            color: 'var(--muted-fg)',
          }}>
            <span>Height</span>
            <span>Link Hash</span>
            <span>Status</span>
            <span style={{ 'text-align': 'right' }}>Date</span>
          </div>

          <For each={entries()}>
            {(entry, index) => (
              <div
                onClick={() => goToVersion(entry)}
                style={{
                  display: 'grid',
                  'grid-template-columns': '60px 1fr 100px 80px',
                  padding: '0.75rem 1rem',
                  'border-bottom': index() < entries().length - 1 ? '1px solid var(--border)' : 'none',
                  'align-items': 'center',
                  cursor: 'pointer',
                  transition: 'background 0.1s ease',
                }}
              >
                {/* Height */}
                <span style={{
                  'font-size': '0.875rem',
                  'font-weight': '600',
                  'font-family': 'monospace',
                }}>
                  #{entry.height}
                </span>

                {/* Link hash (truncated) */}
                <span style={{
                  'font-size': '0.75rem',
                  'font-family': 'monospace',
                  color: 'var(--muted-fg)',
                  overflow: 'hidden',
                  'text-overflow': 'ellipsis',
                  'white-space': 'nowrap',
                }}>
                  {entry.link_hash.substring(0, 24)}...
                </span>

                {/* Published badge */}
                <div>
                  <Show when={entry.published}>
                    <span style={{
                      'font-size': '0.6875rem',
                      'font-weight': '500',
                      padding: '0.125rem 0.5rem',
                      'border-radius': '9999px',
                      background: 'hsl(142 76% 36% / 0.12)',
                      color: 'var(--accent-green)',
                    }}>
                      Published
                    </span>
                  </Show>
                  <Show when={!entry.published}>
                    <span style={{
                      'font-size': '0.6875rem',
                      'font-weight': '500',
                      padding: '0.125rem 0.5rem',
                      'border-radius': '9999px',
                      background: 'var(--muted)',
                      color: 'var(--muted-fg)',
                    }}>
                      Draft
                    </span>
                  </Show>
                </div>

                {/* Date */}
                <span style={{
                  'font-size': '0.75rem',
                  color: 'var(--muted-fg)',
                  'text-align': 'right',
                }}>
                  {new Date(entry.created_at).toLocaleDateString()}
                </span>
              </div>
            )}
          </For>
        </div>

        {/* Pagination */}
        <div style={{
          display: 'flex',
          gap: '0.5rem',
          'justify-content': 'center',
          'margin-top': '1rem',
        }}>
          <Show when={page() > 0}>
            <button
              onClick={() => loadPage(page() - 1)}
              style={paginationBtnStyle()}
            >
              Previous
            </button>
          </Show>
          <Show when={hasMore()}>
            <button
              onClick={() => loadPage(page() + 1)}
              style={paginationBtnStyle()}
            >
              Next
            </button>
          </Show>
        </div>
      </Show>
    </div>
  );
};

function paginationBtnStyle(): Record<string, string> {
  return {
    padding: '0.375rem 0.75rem',
    'border-radius': '6px',
    border: '1px solid var(--border)',
    background: 'var(--muted)',
    color: 'var(--fg)',
    cursor: 'pointer',
    'font-size': '0.8125rem',
    'font-family': 'inherit',
  };
}

export default History;
