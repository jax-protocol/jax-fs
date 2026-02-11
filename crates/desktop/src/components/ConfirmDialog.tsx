import { Component, Show } from 'solid-js';

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  confirmColor?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

const ConfirmDialog: Component<ConfirmDialogProps> = (props) => {
  return (
    <Show when={props.open}>
      <div
        style={{
          position: 'fixed',
          inset: '0',
          background: 'rgba(0, 0, 0, 0.5)',
          display: 'flex',
          'align-items': 'center',
          'justify-content': 'center',
          'z-index': '1000',
        }}
        onClick={(e) => {
          if (e.target === e.currentTarget) props.onCancel();
        }}
      >
        <div style={{
          background: 'var(--bg)',
          border: '1px solid var(--border)',
          'border-radius': 'var(--radius)',
          padding: '1.5rem',
          'min-width': '360px',
          'max-width': '480px',
          'box-shadow': 'var(--shadow-hover)',
        }}>
          <h3 style={{
            'font-size': '1rem',
            'font-weight': '600',
            'margin-bottom': '0.75rem',
          }}>
            {props.title}
          </h3>
          <p style={{
            'font-size': '0.875rem',
            color: 'var(--muted-fg)',
            'margin-bottom': '1.5rem',
            'line-height': '1.5',
          }}>
            {props.message}
          </p>
          <div style={{
            display: 'flex',
            'justify-content': 'flex-end',
            gap: '0.5rem',
          }}>
            <button
              onClick={props.onCancel}
              style={{
                padding: '0.5rem 1rem',
                'border-radius': '8px',
                border: '1px solid var(--border)',
                background: 'var(--muted)',
                color: 'var(--fg)',
                cursor: 'pointer',
                'font-size': '0.875rem',
                'font-family': 'inherit',
              }}
            >
              Cancel
            </button>
            <button
              onClick={props.onConfirm}
              style={{
                padding: '0.5rem 1rem',
                'border-radius': '8px',
                border: `1px solid ${props.confirmColor || 'var(--accent-red)'}`,
                background: props.confirmColor || 'var(--accent-red)',
                color: '#ffffff',
                cursor: 'pointer',
                'font-size': '0.875rem',
                'font-weight': '500',
                'font-family': 'inherit',
              }}
            >
              {props.confirmLabel || 'Delete'}
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default ConfirmDialog;
