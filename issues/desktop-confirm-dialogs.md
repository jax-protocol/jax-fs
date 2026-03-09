# Desktop: native confirm() fires actions before user responds

**Status:** Open
**Priority:** High
**Category:** Bug

## Problem

Several actions in the desktop app use the browser's native `confirm()` dialog for confirmation. In Tauri's webview, `confirm()` does not block JavaScript execution the way it does in a standard browser. The API call fires immediately while the confirmation dialog is still visible, meaning the action completes regardless of whether the user confirms or cancels.

## Affected Actions

- **Remove peer** (`SharePanel.tsx` line 65) — `confirm()` then `removeShare()`
- **Delete mount** (`Mounts.tsx` line 91) — `confirm()` then `deleteMount()`

## Expected Behavior

The action should only execute after the user explicitly confirms in the dialog.

## Fix

Replace native `confirm()` with the `ConfirmDialog` component + signal pattern already used correctly elsewhere in the app (Explorer.tsx uses this for file delete, publish, and unpublish):

```tsx
// Instead of:
const handleRemove = async (publicKey: string) => {
  if (!confirm(`Remove peer?`)) return;
  await removeShare(props.bucketId, publicKey);
};

// Use:
const [removeTarget, setRemoveTarget] = createSignal<string | null>(null);

const handleRemove = async () => {
  const key = removeTarget();
  if (!key) return;
  setRemoveTarget(null);
  await removeShare(props.bucketId, key);
  await loadShares();
};

// In JSX:
<button onClick={() => setRemoveTarget(share.public_key)}>Remove</button>

<ConfirmDialog
  open={!!removeTarget()}
  title="Remove peer"
  message={`Remove peer ${removeTarget()?.substring(0, 16)}... from this bucket?`}
  onConfirm={handleRemove}
  onCancel={() => setRemoveTarget(null)}
/>
```

Apply the same pattern to `Mounts.tsx` `handleDelete`.

## Files to Modify

1. `crates/desktop/src/components/SharePanel.tsx` — replace `confirm()` with `ConfirmDialog`
2. `crates/desktop/src/pages/Mounts.tsx` — replace `confirm()` with `ConfirmDialog`
