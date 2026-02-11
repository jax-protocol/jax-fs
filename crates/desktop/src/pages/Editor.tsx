import { Component, createSignal, onMount, Show, createMemo } from 'solid-js';
import { useParams, useSearchParams, useNavigate } from '@solidjs/router';
import { cat, updateFile } from '../lib/api';
import { bytesToString, isTextMime, pathToBreadcrumbs } from '../lib/utils';
import Breadcrumb from '../components/Breadcrumb';

const Editor: Component = () => {
  const params = useParams<{ bucketId: string }>();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();

  const filePath = () => (searchParams.path as string) || '/';
  const fileName = () => filePath().split('/').pop() || '';

  const [originalContent, setOriginalContent] = createSignal('');
  const [editedContent, setEditedContent] = createSignal('');
  const [loading, setLoading] = createSignal(true);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [saveSuccess, setSaveSuccess] = createSignal(false);

  const isDirty = createMemo(() => editedContent() !== originalContent());

  const fetchContent = async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await cat(params.bucketId, filePath());
      if (!isTextMime(result.mime_type)) {
        setError('This file is not a text file and cannot be edited.');
        return;
      }
      const text = bytesToString(result.content);
      setOriginalContent(text);
      setEditedContent(text);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(() => {
    fetchContent();
  });

  const handleSave = async () => {
    try {
      setSaving(true);
      setError(null);
      setSaveSuccess(false);
      const encoder = new TextEncoder();
      const bytes = Array.from(encoder.encode(editedContent()));
      await updateFile(params.bucketId, filePath(), bytes);
      setOriginalContent(editedContent());
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleDiscard = () => {
    setEditedContent(originalContent());
  };

  const parentDir = () => {
    const parts = filePath().split('/').filter(Boolean);
    if (parts.length <= 1) return '/';
    return '/' + parts.slice(0, -1).join('/');
  };

  const breadcrumbs = () => pathToBreadcrumbs(filePath());

  return (
    <div style={{ display: 'flex', 'flex-direction': 'column', height: 'calc(100vh - 4rem)' }}>
      {/* Header */}
      <div style={{ 'margin-bottom': '1rem' }}>
        <div style={{
          display: 'flex',
          'align-items': 'center',
          gap: '0.75rem',
          'margin-bottom': '0.5rem',
        }}>
          <h2 style={{ 'font-size': '1.5rem', 'font-weight': '700' }}>
            {fileName()}
          </h2>
          <Show when={isDirty()}>
            <span style={{
              'font-size': '0.6875rem',
              'font-weight': '600',
              padding: '0.125rem 0.5rem',
              'border-radius': '9999px',
              background: 'hsl(38 92% 50% / 0.12)',
              color: '#d97706',
            }}>
              Unsaved
            </span>
          </Show>
          <Show when={saveSuccess()}>
            <span style={{
              'font-size': '0.6875rem',
              'font-weight': '600',
              padding: '0.125rem 0.5rem',
              'border-radius': '9999px',
              background: 'hsl(142 76% 36% / 0.12)',
              color: 'var(--accent-green)',
            }}>
              Saved
            </span>
          </Show>
        </div>
        <Breadcrumb
          items={breadcrumbs()}
          onNavigate={(path) => navigate(`/buckets/${params.bucketId}?path=${encodeURIComponent(path)}`)}
        />
      </div>

      {/* Toolbar */}
      <div style={{
        display: 'flex',
        gap: '0.5rem',
        'margin-bottom': '1rem',
        'align-items': 'center',
      }}>
        <button
          onClick={() => navigate(`/buckets/${params.bucketId}?path=${encodeURIComponent(parentDir())}`)}
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
        <button
          onClick={handleSave}
          disabled={!isDirty() || saving()}
          style={{
            padding: '0.5rem 0.75rem',
            'border-radius': '8px',
            border: '1px solid var(--border)',
            background: isDirty() ? 'var(--fg)' : 'var(--muted)',
            color: isDirty() ? 'var(--bg)' : 'var(--muted-fg)',
            cursor: isDirty() && !saving() ? 'pointer' : 'not-allowed',
            'font-size': '0.8125rem',
            'font-weight': '500',
            'font-family': 'inherit',
            opacity: !isDirty() || saving() ? '0.5' : '1',
          }}
        >
          {saving() ? 'Saving...' : 'Save'}
        </button>
        <button
          onClick={handleDiscard}
          disabled={!isDirty()}
          style={{
            padding: '0.5rem 0.75rem',
            'border-radius': '8px',
            border: '1px solid var(--border)',
            background: 'var(--muted)',
            color: isDirty() ? 'var(--fg)' : 'var(--muted-fg)',
            cursor: isDirty() ? 'pointer' : 'not-allowed',
            'font-size': '0.8125rem',
            'font-family': 'inherit',
            opacity: isDirty() ? '1' : '0.5',
          }}
        >
          Discard
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
        <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>Loading file...</p>
      </Show>

      {/* Editor area */}
      <Show when={!loading() && !error()}>
        <textarea
          value={editedContent()}
          onInput={(e) => setEditedContent(e.currentTarget.value)}
          style={{
            flex: '1',
            'font-family': 'monospace',
            'font-size': '0.8125rem',
            'line-height': '1.6',
            padding: '1rem',
            border: '1px solid var(--border)',
            'border-radius': 'var(--radius)',
            background: 'var(--bg)',
            color: 'var(--fg)',
            resize: 'none',
            outline: 'none',
            'tab-size': '2',
          }}
          spellcheck={false}
        />
      </Show>
    </div>
  );
};

export default Editor;
