import { Component, createSignal, createEffect, Show, For } from 'solid-js';
import { getBucketShares, shareBucket, pingPeer, removeShare, ShareInfo } from '../lib/api';
import ConfirmDialog from './ConfirmDialog';

interface SharePanelProps {
  bucketId: string;
  open: boolean;
  onClose: () => void;
}

const SharePanel: Component<SharePanelProps> = (props) => {
  const [shares, setShares] = createSignal<ShareInfo[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // Add peer form
  const [peerKey, setPeerKey] = createSignal('');
  const [role, setRole] = createSignal('owner');
  const [sharing, setSharing] = createSignal(false);
  const [shareSuccess, setShareSuccess] = createSignal<string | null>(null);

  // Remove
  const [removeTarget, setRemoveTarget] = createSignal<string | null>(null);
  const [removingKey, setRemovingKey] = createSignal<string | null>(null);

  // Ping
  const [pingKey, setPingKey] = createSignal<string | null>(null);
  const [pingResult, setPingResult] = createSignal<string | null>(null);
  const [pinging, setPinging] = createSignal(false);

  const loadShares = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await getBucketShares(props.bucketId);
      setShares(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  createEffect(() => {
    if (props.open) loadShares();
  });

  const handleShare = async () => {
    const key = peerKey().trim();
    if (!key) return;
    setSharing(true);
    setError(null);
    setShareSuccess(null);
    try {
      await shareBucket(props.bucketId, key, role());
      setShareSuccess(`Shared with peer as ${role()}`);
      setPeerKey('');
      await loadShares();
    } catch (e) {
      setError(String(e));
    } finally {
      setSharing(false);
    }
  };

  const handleRemove = async () => {
    const publicKey = removeTarget();
    if (!publicKey) return;
    setRemoveTarget(null);
    setRemovingKey(publicKey);
    setError(null);
    try {
      await removeShare(props.bucketId, publicKey);
      await loadShares();
    } catch (e) {
      setError(String(e));
    } finally {
      setRemovingKey(null);
    }
  };

  const handlePing = async (publicKey: string) => {
    setPingKey(publicKey);
    setPinging(true);
    setPingResult(null);
    try {
      const result = await pingPeer(props.bucketId, publicKey);
      setPingResult(result);
    } catch (e) {
      setPingResult(`Error: ${e}`);
    } finally {
      setPinging(false);
    }
  };

  return (
    <Show when={props.open}>
      <div style={{
        position: 'fixed',
        top: '0',
        right: '0',
        bottom: '0',
        width: '380px',
        background: 'var(--bg)',
        'border-left': '1px solid var(--border)',
        padding: '1.5rem',
        overflow: 'auto',
        'z-index': '100',
        'box-shadow': '-4px 0 12px rgba(0,0,0,0.08)',
      }}>
        {/* Header */}
        <div style={{
          display: 'flex',
          'justify-content': 'space-between',
          'align-items': 'center',
          'margin-bottom': '1.5rem',
        }}>
          <h3 style={{ 'font-size': '1rem', 'font-weight': '600' }}>Sharing</h3>
          <button
            onClick={props.onClose}
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--muted-fg)',
              cursor: 'pointer',
              'font-size': '1.25rem',
              padding: '0.25rem',
              'line-height': '1',
            }}
          >
            &times;
          </button>
        </div>

        <Show when={error()}>
          <div style={{
            padding: '0.5rem 0.75rem',
            'border-radius': '6px',
            background: 'hsl(0 84% 60% / 0.08)',
            border: '1px solid hsl(0 84% 60% / 0.3)',
            color: 'var(--accent-red)',
            'font-size': '0.8125rem',
            'margin-bottom': '1rem',
          }}>
            {error()}
          </div>
        </Show>

        <Show when={shareSuccess()}>
          <div style={{
            padding: '0.5rem 0.75rem',
            'border-radius': '6px',
            background: 'hsl(142 76% 36% / 0.08)',
            border: '1px solid hsl(142 76% 36% / 0.3)',
            color: 'var(--accent-green)',
            'font-size': '0.8125rem',
            'margin-bottom': '1rem',
          }}>
            {shareSuccess()}
          </div>
        </Show>

        {/* Current shares */}
        <div style={{ 'margin-bottom': '1.5rem' }}>
          <div style={labelStyle()}>Peers</div>

          <Show when={loading()}>
            <p style={{ color: 'var(--muted-fg)', 'font-size': '0.8125rem' }}>Loading...</p>
          </Show>

          <Show when={!loading() && shares().length === 0}>
            <p style={{ color: 'var(--muted-fg)', 'font-size': '0.8125rem' }}>No shares yet</p>
          </Show>

          <Show when={!loading() && shares().length > 0}>
            <div style={{
              border: '1px solid var(--border)',
              'border-radius': '8px',
              overflow: 'hidden',
            }}>
              <For each={shares()}>
                {(share, index) => (
                  <div style={{
                    padding: '0.625rem 0.75rem',
                    'border-bottom': index() < shares().length - 1 ? '1px solid var(--border)' : 'none',
                    display: 'flex',
                    'align-items': 'center',
                    gap: '0.5rem',
                  }}>
                    <div style={{ flex: '1', 'min-width': '0' }}>
                      <div style={{
                        'font-size': '0.6875rem',
                        'font-family': 'monospace',
                        'word-break': 'break-all',
                        color: share.is_self ? 'var(--fg)' : 'var(--muted-fg)',
                      }}>
                        {share.public_key.substring(0, 16)}...
                        {share.is_self && (
                          <span style={{
                            color: 'var(--accent-blue)',
                            'margin-left': '0.375rem',
                            'font-weight': '500',
                          }}>
                            (you)
                          </span>
                        )}
                      </div>
                    </div>
                    <span style={{
                      'font-size': '0.625rem',
                      'font-weight': '500',
                      padding: '0.125rem 0.375rem',
                      'border-radius': '9999px',
                      background: share.role === 'Owner'
                        ? 'hsl(142 76% 36% / 0.12)'
                        : 'hsl(217 91% 60% / 0.08)',
                      color: share.role === 'Owner'
                        ? 'var(--accent-green)'
                        : 'var(--accent-blue)',
                      'flex-shrink': '0',
                    }}>
                      {share.role}
                    </span>
                    <Show when={!share.is_self}>
                      <button
                        onClick={() => handlePing(share.public_key)}
                        disabled={pinging() && pingKey() === share.public_key}
                        style={{
                          background: 'none',
                          border: '1px solid var(--border)',
                          color: 'var(--muted-fg)',
                          cursor: 'pointer',
                          'font-size': '0.625rem',
                          'font-family': 'inherit',
                          padding: '0.125rem 0.375rem',
                          'border-radius': '4px',
                          'flex-shrink': '0',
                        }}
                      >
                        {pinging() && pingKey() === share.public_key ? '...' : 'Ping'}
                      </button>
                      <button
                        onClick={() => setRemoveTarget(share.public_key)}
                        disabled={removingKey() === share.public_key}
                        style={{
                          background: 'none',
                          border: '1px solid hsl(0 84% 60% / 0.3)',
                          color: 'var(--accent-red)',
                          cursor: removingKey() === share.public_key ? 'not-allowed' : 'pointer',
                          'font-size': '0.625rem',
                          'font-family': 'inherit',
                          padding: '0.125rem 0.375rem',
                          'border-radius': '4px',
                          'flex-shrink': '0',
                          opacity: removingKey() === share.public_key ? '0.4' : '1',
                        }}
                      >
                        {removingKey() === share.public_key ? '...' : 'Remove'}
                      </button>
                    </Show>
                  </div>
                )}
              </For>
            </div>

            <Show when={pingResult()}>
              <div style={{
                'margin-top': '0.5rem',
                padding: '0.375rem 0.625rem',
                'border-radius': '6px',
                background: 'var(--muted)',
                'font-size': '0.75rem',
                color: 'var(--muted-fg)',
              }}>
                {pingResult()}
              </div>
            </Show>
          </Show>
        </div>

        {/* Add peer form */}
        <div>
          <div style={labelStyle()}>Add Peer</div>
          <div style={{ display: 'flex', 'flex-direction': 'column', gap: '0.5rem' }}>
            <input
              type="text"
              placeholder="Peer public key (hex)"
              value={peerKey()}
              onInput={(e) => setPeerKey(e.currentTarget.value)}
              style={inputStyle()}
            />
            <div style={{ display: 'flex', gap: '0.5rem', 'align-items': 'center' }}>
              <label style={{ 'font-size': '0.8125rem', color: 'var(--muted-fg)' }}>Role:</label>
              <select
                value={role()}
                onChange={(e) => setRole(e.currentTarget.value)}
                style={{
                  padding: '0.375rem 0.625rem',
                  'border-radius': '6px',
                  border: '1px solid var(--border)',
                  background: 'var(--bg)',
                  color: 'var(--fg)',
                  'font-size': '0.8125rem',
                  'font-family': 'inherit',
                  outline: 'none',
                }}
              >
                <option value="owner">Owner</option>
                <option value="mirror">Mirror</option>
              </select>
            </div>
            <button
              onClick={handleShare}
              disabled={sharing() || !peerKey().trim()}
              style={{
                padding: '0.5rem 0.75rem',
                'border-radius': '8px',
                border: '1px solid var(--border)',
                background: 'var(--fg)',
                color: 'var(--bg)',
                cursor: sharing() || !peerKey().trim() ? 'not-allowed' : 'pointer',
                'font-size': '0.8125rem',
                'font-weight': '500',
                'font-family': 'inherit',
                opacity: sharing() || !peerKey().trim() ? '0.4' : '1',
                'align-self': 'flex-start',
              }}
            >
              {sharing() ? 'Sharing...' : 'Share'}
            </button>
          </div>
        </div>
      </div>

      <ConfirmDialog
        open={!!removeTarget()}
        title="Remove peer"
        message={`Remove peer ${removeTarget()?.substring(0, 16)}... from this bucket?`}
        confirmLabel="Remove"
        onConfirm={handleRemove}
        onCancel={() => setRemoveTarget(null)}
      />
    </Show>
  );
};

function labelStyle(): Record<string, string> {
  return {
    'font-size': '0.6875rem',
    'font-weight': '600',
    'text-transform': 'uppercase',
    'letter-spacing': '0.05em',
    color: 'var(--muted-fg)',
    'margin-bottom': '0.5rem',
  };
}

function inputStyle(): Record<string, string> {
  return {
    padding: '0.5rem 0.75rem',
    'border-radius': '8px',
    border: '1px solid var(--border)',
    background: 'var(--bg)',
    color: 'var(--fg)',
    'font-size': '0.8125rem',
    'font-family': 'inherit',
    outline: 'none',
  };
}

export default SharePanel;
