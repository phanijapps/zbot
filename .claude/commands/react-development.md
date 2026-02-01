---
description: Modern React/TypeScript development patterns and best practices
---

# React Development Guide

Use this skill for React/TypeScript development. Covers modern patterns, hooks, state management, and performance optimization.

## Project Structure

```
src/
├── components/        # Reusable UI components
├── features/          # Feature modules (co-located logic + UI)
├── hooks/             # Custom hooks
├── services/          # API clients, external integrations
├── utils/             # Pure utility functions
├── types/             # Shared TypeScript types
└── App.tsx
```

**Principle**: Co-locate related code. Feature folders contain components, hooks, types, and tests for that feature.

## Component Patterns

### Functional Components with TypeScript

```tsx
interface ButtonProps {
  label: string;
  onClick: () => void;
  variant?: "primary" | "secondary" | "danger";
  disabled?: boolean;
  children?: React.ReactNode;
}

export function Button({
  label,
  onClick,
  variant = "primary",
  disabled = false,
  children,
}: ButtonProps) {
  return (
    <button
      className={`btn btn--${variant}`}
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
    >
      {children || label}
    </button>
  );
}
```

### Component Composition

```tsx
// Compound components for complex UI
function Card({ children }: { children: React.ReactNode }) {
  return <div className="card">{children}</div>;
}

Card.Header = function CardHeader({ children }: { children: React.ReactNode }) {
  return <div className="card__header">{children}</div>;
};

Card.Body = function CardBody({ children }: { children: React.ReactNode }) {
  return <div className="card__body">{children}</div>;
};

// Usage
<Card>
  <Card.Header>Title</Card.Header>
  <Card.Body>Content</Card.Body>
</Card>
```

## Hooks Best Practices

### useState

```tsx
// Primitive state
const [count, setCount] = useState(0);

// Object state - always spread previous state
const [form, setForm] = useState({ name: "", email: "" });
setForm(prev => ({ ...prev, name: "John" }));

// Lazy initialization for expensive computations
const [data, setData] = useState(() => computeExpensiveValue());
```

### useEffect

```tsx
// Dependency array rules:
// - [] = run once on mount
// - [dep] = run when dep changes
// - no array = run every render (usually wrong)

useEffect(() => {
  const subscription = api.subscribe(handler);

  // Always clean up subscriptions
  return () => subscription.unsubscribe();
}, [handler]);

// Async in useEffect
useEffect(() => {
  let cancelled = false;

  async function fetchData() {
    const result = await api.getData();
    if (!cancelled) {
      setData(result);
    }
  }

  fetchData();

  return () => { cancelled = true; };
}, []);
```

### useCallback and useMemo

```tsx
// useCallback for stable function references
const handleClick = useCallback((id: string) => {
  setSelected(id);
}, []); // Empty deps = stable reference

// useMemo for expensive computations
const sortedItems = useMemo(() => {
  return items.sort((a, b) => a.name.localeCompare(b.name));
}, [items]);

// Rule: Only optimize when you have a measured performance problem
```

### Custom Hooks

```tsx
// Extract reusable logic into custom hooks
function useAsync<T>(asyncFn: () => Promise<T>, deps: unknown[] = []) {
  const [state, setState] = useState<{
    data: T | null;
    loading: boolean;
    error: Error | null;
  }>({ data: null, loading: true, error: null });

  useEffect(() => {
    let cancelled = false;

    setState(s => ({ ...s, loading: true }));

    asyncFn()
      .then(data => {
        if (!cancelled) setState({ data, loading: false, error: null });
      })
      .catch(error => {
        if (!cancelled) setState({ data: null, loading: false, error });
      });

    return () => { cancelled = true; };
  }, deps);

  return state;
}

// Usage
const { data, loading, error } = useAsync(() => fetchUser(id), [id]);
```

## State Management

### Local State First

Start with local state. Lift state up only when needed.

```tsx
// Parent manages shared state
function Parent() {
  const [selected, setSelected] = useState<string | null>(null);

  return (
    <>
      <List onSelect={setSelected} />
      <Detail id={selected} />
    </>
  );
}
```

### Context for Global State

```tsx
interface AuthContextType {
  user: User | null;
  login: (credentials: Credentials) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextType | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<User | null>(null);

  const login = useCallback(async (credentials: Credentials) => {
    const user = await authApi.login(credentials);
    setUser(user);
  }, []);

  const logout = useCallback(() => {
    setUser(null);
    authApi.logout();
  }, []);

  return (
    <AuthContext.Provider value={{ user, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) throw new Error("useAuth must be within AuthProvider");
  return context;
}
```

## Error Handling

### Error Boundaries

```tsx
class ErrorBoundary extends React.Component<
  { children: React.ReactNode; fallback: React.ReactNode },
  { hasError: boolean }
> {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("Error:", error, info);
  }

  render() {
    if (this.state.hasError) return this.props.fallback;
    return this.props.children;
  }
}

// Usage
<ErrorBoundary fallback={<ErrorPage />}>
  <App />
</ErrorBoundary>
```

### Async Error Handling

```tsx
function DataLoader() {
  const [data, setData] = useState<Data | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchData()
      .then(setData)
      .catch(e => setError(e.message))
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <Spinner />;
  if (error) return <ErrorMessage error={error} />;
  if (!data) return <Empty />;
  return <DataView data={data} />;
}
```

## Performance Patterns

### Virtualization for Long Lists

```tsx
import { useVirtualizer } from "@tanstack/react-virtual";

function VirtualList({ items }: { items: Item[] }) {
  const parentRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 50,
  });

  return (
    <div ref={parentRef} style={{ height: 400, overflow: "auto" }}>
      <div style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map(virtualRow => (
          <div
            key={virtualRow.key}
            style={{
              position: "absolute",
              top: virtualRow.start,
              height: virtualRow.size,
            }}
          >
            <ItemRow item={items[virtualRow.index]} />
          </div>
        ))}
      </div>
    </div>
  );
}
```

### Debouncing Input

```tsx
function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedValue(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);

  return debouncedValue;
}

// Usage
function SearchInput() {
  const [query, setQuery] = useState("");
  const debouncedQuery = useDebounce(query, 300);

  useEffect(() => {
    if (debouncedQuery) {
      search(debouncedQuery);
    }
  }, [debouncedQuery]);

  return <input value={query} onChange={e => setQuery(e.target.value)} />;
}
```

## Testing Patterns

```tsx
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

describe("Button", () => {
  it("calls onClick when clicked", async () => {
    const handleClick = vi.fn();
    render(<Button label="Click me" onClick={handleClick} />);

    await userEvent.click(screen.getByRole("button", { name: /click me/i }));

    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it("disables button when disabled prop is true", () => {
    render(<Button label="Submit" onClick={() => {}} disabled />);

    expect(screen.getByRole("button")).toBeDisabled();
  });
});

// Testing async components
describe("DataLoader", () => {
  it("shows data after loading", async () => {
    vi.spyOn(api, "fetchData").mockResolvedValue({ name: "Test" });

    render(<DataLoader />);

    expect(screen.getByText(/loading/i)).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText("Test")).toBeInTheDocument();
    });
  });
});
```

## Accessibility Checklist

- Use semantic HTML (`button`, `nav`, `main`, `article`)
- Add `aria-label` for icon-only buttons
- Ensure keyboard navigation works
- Maintain focus management in modals
- Use `role` attributes when semantic HTML isn't enough
- Test with screen reader
- Ensure sufficient color contrast

## Common Pitfalls

1. **Missing keys in lists**: Always use stable, unique keys
2. **Stale closures**: Dependencies in useEffect/useCallback
3. **Mutating state directly**: Always create new objects/arrays
4. **Over-rendering**: Use React DevTools Profiler to identify
5. **Memory leaks**: Clean up subscriptions, cancel async operations
6. **Prop drilling**: Use context or composition instead
