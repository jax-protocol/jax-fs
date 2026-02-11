import { Component, createSignal, onMount, onCleanup, Show, createMemo } from 'solid-js';
import { useParams, useSearchParams, useNavigate } from '@solidjs/router';
import { cat, catAtVersion, CatResult } from '../lib/api';
import {
  bytesToString, bytesToDataUrl, bytesToBlobUrl, bytesToHexDump,
  isTextMime, isImageMime, isVideoMime, isAudioMime, formatFileSize, pathToBreadcrumbs,
} from '../lib/utils';
import Breadcrumb from '../components/Breadcrumb';

const Viewer: Component = () => {
  const params = useParams<{ bucketId: string }>();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();

  const filePath = () => (searchParams.path as string) || '/';
  const fileName = () => filePath().split('/').pop() || '';
  const versionHash = () => (searchParams.at as string) || null;
  const isHistoryView = createMemo(() => !!versionHash());

  const [result, setResult] = createSignal<CatResult | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  // Track blob URLs for cleanup
  let blobUrls: string[] = [];

  const fetchContent = async () => {
    try {
      setLoading(true);
      setError(null);
      let data: CatResult;
      if (versionHash()) {
        data = await catAtVersion(params.bucketId, versionHash()!, filePath());
      } else {
        data = await cat(params.bucketId, filePath());
      }
      setResult(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(() => {
    fetchContent();
  });

  onCleanup(() => {
    // Revoke blob URLs to prevent memory leaks
    blobUrls.forEach(url => URL.revokeObjectURL(url));
  });

  const isText = createMemo(() => {
    const r = result();
    return r ? isTextMime(r.mime_type) : false;
  });

  const isImage = createMemo(() => {
    const r = result();
    return r ? isImageMime(r.mime_type) : false;
  });

  const isVideo = createMemo(() => {
    const r = result();
    return r ? isVideoMime(r.mime_type) : false;
  });

  const isAudio = createMemo(() => {
    const r = result();
    return r ? isAudioMime(r.mime_type) : false;
  });

  const isMarkdown = createMemo(() => {
    const r = result();
    return r?.mime_type === 'text/markdown';
  });

  const textContent = createMemo(() => {
    const r = result();
    if (!r || !isText()) return '';
    return bytesToString(r.content);
  });

  const imageDataUrl = createMemo(() => {
    const r = result();
    if (!r || !isImage()) return '';
    return bytesToDataUrl(r.content, r.mime_type);
  });

  const mediaBlobUrl = createMemo(() => {
    const r = result();
    if (!r || (!isVideo() && !isAudio())) return '';
    const url = bytesToBlobUrl(r.content, r.mime_type);
    blobUrls.push(url);
    return url;
  });

  const hexDump = createMemo(() => {
    const r = result();
    if (!r) return '';
    return bytesToHexDump(r.content);
  });

  const renderMarkdown = (text: string): string => {
    let html = text
      // Code blocks (must come before inline code)
      .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>')
      // Headings
      .replace(/^### (.+)$/gm, '<h3>$1</h3>')
      .replace(/^## (.+)$/gm, '<h2>$1</h2>')
      .replace(/^# (.+)$/gm, '<h1>$1</h1>')
      // Bold and italic
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/\*(.+?)\*/g, '<em>$1</em>')
      // Inline code
      .replace(/`(.+?)`/g, '<code style="background:var(--muted);padding:0.125rem 0.25rem;border-radius:3px;font-size:0.8125rem">$1</code>')
      // Links
      .replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2" style="color:var(--accent-blue)">$1</a>')
      // Unordered lists
      .replace(/^[*-] (.+)$/gm, '<li>$1</li>')
      // Paragraphs (blank line separated)
      .replace(/\n\n/g, '</p><p>')
      // Line breaks
      .replace(/\n/g, '<br/>');

    // Wrap consecutive <li> tags in <ul>
    html = html.replace(/((?:<li>.*?<\/li>(?:<br\/>)?)+)/g, '<ul style="padding-left:1.25rem;margin:0.5rem 0">$1</ul>');

    return `<p>${html}</p>`;
  };

  const parentDir = () => {
    const parts = filePath().split('/').filter(Boolean);
    if (parts.length <= 1) return '/';
    return '/' + parts.slice(0, -1).join('/');
  };

  const breadcrumbs = () => pathToBreadcrumbs(filePath());

  const explorerUrl = () => {
    const base = `/buckets/${params.bucketId}?path=${encodeURIComponent(parentDir())}`;
    return versionHash() ? `${base}&at=${encodeURIComponent(versionHash()!)}` : base;
  };

  return (
    <div>
      {/* History banner */}
      <Show when={isHistoryView()}>
        <div style={{
          background: 'hsl(217 91% 60% / 0.08)',
          border: '1px solid hsl(217 91% 60% / 0.3)',
          padding: '0.625rem 1rem',
          'border-radius': '8px',
          'margin-bottom': '1rem',
          'font-size': '0.875rem',
          color: 'var(--accent-blue)',
        }}>
          Viewing historical version: <code style={{ 'font-size': '0.75rem' }}>{versionHash()!.substring(0, 16)}...</code>
        </div>
      </Show>

      {/* Header */}
      <div style={{ 'margin-bottom': '1rem' }}>
        <h2 style={{ 'font-size': '1.5rem', 'font-weight': '700', 'margin-bottom': '0.5rem' }}>
          {fileName()}
        </h2>
        <Breadcrumb
          items={breadcrumbs()}
          onNavigate={(path) => navigate(`/buckets/${params.bucketId}?path=${encodeURIComponent(path)}`)}
        />
      </div>

      {/* Action bar */}
      <div style={{
        display: 'flex',
        gap: '0.5rem',
        'margin-bottom': '1rem',
        'align-items': 'center',
      }}>
        <button
          onClick={() => navigate(explorerUrl())}
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
        <Show when={isText() && !isHistoryView()}>
          <button
            onClick={() => navigate(`/buckets/${params.bucketId}/edit?path=${encodeURIComponent(filePath())}`)}
            style={{
              padding: '0.5rem 0.75rem',
              'border-radius': '8px',
              border: '1px solid var(--border)',
              background: 'var(--fg)',
              color: 'var(--bg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-weight': '500',
              'font-family': 'inherit',
            }}
          >
            Edit
          </button>
        </Show>
        <Show when={result()}>
          <span style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)', 'margin-left': 'auto' }}>
            {result()!.mime_type} &middot; {formatFileSize(result()!.size)}
          </span>
        </Show>
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

      {/* Content */}
      <Show when={!loading() && result()}>
        <div style={{
          border: '1px solid var(--border)',
          'border-radius': 'var(--radius)',
          overflow: 'hidden',
        }}>
          {/* Markdown rendering */}
          <Show when={isMarkdown()}>
            <div
              style={{
                padding: '1.5rem',
                'font-size': '0.875rem',
                'line-height': '1.7',
              }}
              innerHTML={renderMarkdown(textContent())}
            />
          </Show>

          {/* Plain text */}
          <Show when={isText() && !isMarkdown()}>
            <pre style={{
              padding: '1rem',
              'font-size': '0.8125rem',
              'font-family': 'monospace',
              'line-height': '1.6',
              overflow: 'auto',
              margin: '0',
              'white-space': 'pre-wrap',
              'word-break': 'break-word',
            }}>
              {textContent()}
            </pre>
          </Show>

          {/* Image */}
          <Show when={isImage()}>
            <div style={{
              padding: '1.5rem',
              display: 'flex',
              'justify-content': 'center',
              background: 'var(--muted)',
            }}>
              <img
                src={imageDataUrl()}
                alt={fileName()}
                style={{
                  'max-width': '100%',
                  'max-height': '70vh',
                  'border-radius': '8px',
                }}
              />
            </div>
          </Show>

          {/* Video */}
          <Show when={isVideo()}>
            <div style={{
              padding: '1.5rem',
              display: 'flex',
              'justify-content': 'center',
              background: 'var(--muted)',
            }}>
              <video
                controls
                style={{ 'max-width': '100%', 'max-height': '70vh', 'border-radius': '8px' }}
              >
                <source src={mediaBlobUrl()} type={result()!.mime_type} />
              </video>
            </div>
          </Show>

          {/* Audio */}
          <Show when={isAudio()}>
            <div style={{
              padding: '1.5rem',
              background: 'var(--muted)',
            }}>
              <audio controls style={{ width: '100%' }}>
                <source src={mediaBlobUrl()} type={result()!.mime_type} />
              </audio>
            </div>
          </Show>

          {/* Binary hex dump (fallback for unknown types) */}
          <Show when={!isText() && !isImage() && !isVideo() && !isAudio()}>
            <div style={{ padding: '0.75rem 1rem' }}>
              <div style={{
                'font-size': '0.75rem',
                'font-weight': '600',
                'text-transform': 'uppercase',
                'letter-spacing': '0.05em',
                color: 'var(--muted-fg)',
                'margin-bottom': '0.75rem',
              }}>
                Hex Dump
              </div>
              <pre style={{
                'font-size': '0.75rem',
                'font-family': 'monospace',
                'line-height': '1.5',
                overflow: 'auto',
                margin: '0',
                color: 'var(--muted-fg)',
              }}>
                {hexDump()}
              </pre>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default Viewer;
