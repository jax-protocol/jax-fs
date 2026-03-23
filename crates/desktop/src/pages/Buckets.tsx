import { Component, createSignal, onMount, For, Show } from 'solid-js';
import { A } from '@solidjs/router';
import { listBuckets, createBucket, removeBucket, BucketInfo, isFuseAvailable, isBucketMounted, mountBucket, unmountBucket, MountInfo } from '../lib/api';
import ConfirmDialog from '../components/ConfirmDialog';

const Buckets: Component = () => {
  const [buckets, setBuckets] = createSignal<BucketInfo[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [newBucketName, setNewBucketName] = createSignal('');
  const [creating, setCreating] = createSignal(false);
  const [fuseAvailable, setFuseAvailable] = createSignal(false);
  const [mountStatus, setMountStatus] = createSignal<Record<string, MountInfo | null>>({});
  const [mountingBucket, setMountingBucket] = createSignal<string | null>(null);
  const [removingBucket, setRemovingBucket] = createSignal<BucketInfo | null>(null);

  const fetchBuckets = async () => {
    try {
      setLoading(true);
      const result = await listBuckets();
      setBuckets(result);
      setError(null);

      // Check mount status for each bucket if FUSE is available
      if (fuseAvailable()) {
        const statuses: Record<string, MountInfo | null> = {};
        for (const bucket of result) {
          try {
            statuses[bucket.bucket_id] = await isBucketMounted(bucket.bucket_id);
          } catch {
            statuses[bucket.bucket_id] = null;
          }
        }
        setMountStatus(statuses);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleMount = async (bucketId: string, e: Event) => {
    e.preventDefault();
    e.stopPropagation();

    try {
      setMountingBucket(bucketId);
      setError(null);
      const mount = await mountBucket(bucketId);
      setMountStatus(prev => ({ ...prev, [bucketId]: mount }));
    } catch (err) {
      setError(String(err));
    } finally {
      setMountingBucket(null);
    }
  };

  const handleUnmount = async (bucketId: string, e: Event) => {
    e.preventDefault();
    e.stopPropagation();

    try {
      setMountingBucket(bucketId);
      setError(null);
      await unmountBucket(bucketId);
      setMountStatus(prev => ({ ...prev, [bucketId]: null }));
    } catch (err) {
      setError(String(err));
    } finally {
      setMountingBucket(null);
    }
  };

  const handleCreateBucket = async () => {
    const name = newBucketName().trim();
    if (!name) return;

    try {
      setCreating(true);
      setError(null);
      await createBucket(name);
      setNewBucketName('');
      await fetchBuckets();
    } catch (e) {
      setError(String(e));
    } finally {
      setCreating(false);
    }
  };

  const handleRemoveBucket = async (bucket: BucketInfo, e: Event) => {
    e.preventDefault();
    e.stopPropagation();
    setRemovingBucket(bucket);
  };

  const confirmRemoveBucket = async () => {
    const bucket = removingBucket();
    if (!bucket) return;

    try {
      setError(null);
      await removeBucket(bucket.bucket_id);
      setRemovingBucket(null);
      await fetchBuckets();
    } catch (err) {
      setError(String(err));
      setRemovingBucket(null);
    }
  };

  onMount(async () => {
    const fuse = await isFuseAvailable();
    setFuseAvailable(fuse);
    await fetchBuckets();
  });

  return (
    <div>
      <div style={{
        display: 'flex',
        'justify-content': 'space-between',
        'align-items': 'center',
        'margin-bottom': '1.5rem',
      }}>
        <h2 style={{ 'font-size': '1.5rem', 'font-weight': '700' }}>Buckets</h2>
      </div>

      {/* Create bucket form */}
      <div style={{
        display: 'flex',
        gap: '0.5rem',
        'margin-bottom': '1.5rem',
      }}>
        <input
          type="text"
          placeholder="New bucket name..."
          value={newBucketName()}
          onInput={(e) => setNewBucketName(e.currentTarget.value)}
          onKeyPress={(e) => e.key === 'Enter' && handleCreateBucket()}
          style={{
            flex: 1,
            padding: '0.5rem 0.75rem',
            'border-radius': '8px',
            border: '1px solid var(--border)',
            background: 'var(--bg)',
            color: 'var(--fg)',
            'font-size': '0.875rem',
            'font-family': 'inherit',
            outline: 'none',
          }}
        />
        <button
          onClick={handleCreateBucket}
          disabled={creating() || !newBucketName().trim()}
          style={{
            padding: '0.5rem 1rem',
            'border-radius': '8px',
            border: '1px solid var(--border)',
            background: 'var(--fg)',
            color: 'var(--bg)',
            cursor: creating() || !newBucketName().trim() ? 'not-allowed' : 'pointer',
            'font-size': '0.875rem',
            'font-weight': '500',
            'font-family': 'inherit',
            opacity: creating() || !newBucketName().trim() ? '0.4' : '1',
            transition: 'all 0.15s ease',
          }}
        >
          {creating() ? 'Creating...' : 'Create'}
        </button>
      </div>

      {/* Error display */}
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

      {/* Loading state */}
      <Show when={loading()}>
        <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>Loading buckets...</p>
      </Show>

      {/* Empty state */}
      <Show when={!loading() && buckets().length === 0}>
        <div style={{
          background: 'var(--muted)',
          border: '1px solid var(--border)',
          'border-radius': 'var(--radius)',
          padding: '3rem',
          'text-align': 'center',
        }}>
          <p style={{ 'font-size': '0.9375rem', 'font-weight': '500', 'margin-bottom': '0.375rem' }}>No buckets yet</p>
          <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>Create your first bucket to get started</p>
        </div>
      </Show>

      {/* Bucket grid */}
      <Show when={!loading() && buckets().length > 0}>
        <div style={{
          display: 'grid',
          'grid-template-columns': 'repeat(auto-fill, minmax(280px, 1fr))',
          gap: '1rem',
        }}>
          <For each={buckets()}>
            {(bucket) => (
              <A
                href={`/buckets/${bucket.bucket_id}?path=/`}
                style={{ 'text-decoration': 'none', color: 'inherit' }}
              >
                <div style={{
                  background: 'var(--muted)',
                  border: '1px solid var(--border)',
                  'border-radius': 'var(--radius)',
                  padding: '1.5rem',
                  transition: 'all 0.2s ease',
                  cursor: 'pointer',
                }}>
                  <div style={{
                    display: 'flex',
                    'justify-content': 'space-between',
                    'align-items': 'flex-start',
                    'margin-bottom': '1rem',
                  }}>
                    <h3 style={{ 'font-size': '0.9375rem', 'font-weight': '600' }}>{bucket.name}</h3>
                    <span style={{
                      'font-size': '0.6875rem',
                      'font-weight': '600',
                      padding: '0.25rem 0.5rem',
                      'border-radius': '9999px',
                      background: 'hsl(142 76% 36% / 0.12)',
                      color: 'var(--accent-green)',
                    }}>
                      Owner
                    </span>
                  </div>

                  <div style={{ display: 'flex', 'flex-direction': 'column', gap: '0.375rem' }}>
                    <div style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)', 'font-family': 'monospace' }}>
                      {bucket.bucket_id.substring(0, 8)}...
                    </div>
                    <Show when={bucket.link_hash}>
                      <div style={{
                        'font-size': '0.75rem',
                        color: 'var(--muted-fg)',
                        'font-family': 'monospace',
                      }}>
                        {bucket.link_hash.substring(0, 16)}...
                      </div>
                    </Show>
                  </div>

                  {/* Mount status and button */}
                  <Show when={fuseAvailable()}>
                    <div style={{
                      'margin-top': '0.75rem',
                      'padding-top': '0.75rem',
                      'border-top': '1px solid var(--border)',
                      display: 'flex',
                      'justify-content': 'space-between',
                      'align-items': 'center',
                    }}>
                      <Show when={mountStatus()[bucket.bucket_id]} fallback={
                        <span style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)' }}>
                          Not mounted
                        </span>
                      }>
                        <span style={{
                          'font-size': '0.75rem',
                          color: 'var(--accent-green)',
                          display: 'flex',
                          'align-items': 'center',
                          gap: '0.25rem',
                        }}>
                          <span style={{
                            width: '6px',
                            height: '6px',
                            'border-radius': '50%',
                            background: 'var(--accent-green)',
                          }} />
                          Mounted
                        </span>
                      </Show>
                      <Show when={mountStatus()[bucket.bucket_id]} fallback={
                        <button
                          onClick={(e) => handleMount(bucket.bucket_id, e)}
                          disabled={mountingBucket() === bucket.bucket_id}
                          style={{
                            padding: '0.25rem 0.5rem',
                            'border-radius': '6px',
                            border: '1px solid var(--border)',
                            background: 'var(--bg)',
                            color: 'var(--fg)',
                            cursor: mountingBucket() === bucket.bucket_id ? 'not-allowed' : 'pointer',
                            'font-size': '0.6875rem',
                            'font-weight': '500',
                            'font-family': 'inherit',
                            opacity: mountingBucket() === bucket.bucket_id ? '0.5' : '1',
                          }}
                        >
                          {mountingBucket() === bucket.bucket_id ? 'Mounting...' : 'Mount'}
                        </button>
                      }>
                        <button
                          onClick={(e) => handleUnmount(bucket.bucket_id, e)}
                          disabled={mountingBucket() === bucket.bucket_id}
                          style={{
                            padding: '0.25rem 0.5rem',
                            'border-radius': '6px',
                            border: '1px solid var(--border)',
                            background: 'var(--bg)',
                            color: 'var(--accent-red)',
                            cursor: mountingBucket() === bucket.bucket_id ? 'not-allowed' : 'pointer',
                            'font-size': '0.6875rem',
                            'font-weight': '500',
                            'font-family': 'inherit',
                            opacity: mountingBucket() === bucket.bucket_id ? '0.5' : '1',
                          }}
                        >
                          {mountingBucket() === bucket.bucket_id ? 'Unmounting...' : 'Unmount'}
                        </button>
                      </Show>
                    </div>
                  </Show>

                  <div style={{
                    'margin-top': fuseAvailable() ? '0.5rem' : '1rem',
                    'padding-top': fuseAvailable() ? '0' : '0.75rem',
                    'border-top': fuseAvailable() ? 'none' : '1px solid var(--border)',
                    display: 'flex',
                    'justify-content': 'space-between',
                    'align-items': 'center',
                  }}>
                    <span style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)' }}>
                      {new Date(bucket.created_at).toLocaleDateString()}
                    </span>
                    <div style={{ display: 'flex', gap: '0.5rem', 'align-items': 'center' }}>
                      <button
                        onClick={(e) => handleRemoveBucket(bucket, e)}
                        style={{
                          padding: '0.25rem 0.5rem',
                          'border-radius': '6px',
                          border: '1px solid var(--border)',
                          background: 'var(--bg)',
                          color: 'var(--accent-red)',
                          cursor: 'pointer',
                          'font-size': '0.6875rem',
                          'font-weight': '500',
                          'font-family': 'inherit',
                        }}
                      >
                        Remove
                      </button>
                      <span style={{
                        'font-size': '0.75rem',
                        'font-weight': '500',
                        color: 'var(--fg)',
                      }}>
                        Open
                      </span>
                    </div>
                  </div>
                </div>
              </A>
            )}
          </For>
        </div>
      </Show>

      <ConfirmDialog
        open={removingBucket() !== null}
        title="Remove Bucket"
        message={`Remove "${removingBucket()?.name}" from this node? Sync will stop and any FUSE mounts will be unmounted. The bucket log is preserved.`}
        confirmLabel="Remove"
        onConfirm={confirmRemoveBucket}
        onCancel={() => setRemovingBucket(null)}
      />
    </div>
  );
};

export default Buckets;
