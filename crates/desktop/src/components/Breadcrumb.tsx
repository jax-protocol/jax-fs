import { Component, For } from 'solid-js';
import { Breadcrumb as BreadcrumbItem } from '../lib/utils';

interface BreadcrumbProps {
  items: BreadcrumbItem[];
  onNavigate: (path: string) => void;
}

const Breadcrumb: Component<BreadcrumbProps> = (props) => {
  return (
    <nav style={{
      display: 'flex',
      'align-items': 'center',
      gap: '0.25rem',
      'font-size': '0.875rem',
      'flex-wrap': 'wrap',
    }}>
      <For each={props.items}>
        {(item, index) => (
          <>
            {index() > 1 && (
              <span style={{ color: 'var(--muted-fg)', 'user-select': 'none' }}>/</span>
            )}
            {index() === props.items.length - 1 ? (
              <span style={{ 'font-weight': '500', color: 'var(--fg)' }}>
                {item.label}
              </span>
            ) : (
              <button
                onClick={() => props.onNavigate(item.path)}
                style={{
                  background: 'none',
                  border: 'none',
                  color: 'var(--muted-fg)',
                  cursor: 'pointer',
                  padding: '0.125rem 0.25rem',
                  'border-radius': '4px',
                  'font-size': '0.875rem',
                  'font-family': 'inherit',
                  transition: 'color 0.15s ease',
                }}
              >
                {item.label}
              </button>
            )}
          </>
        )}
      </For>
    </nav>
  );
};

export default Breadcrumb;
