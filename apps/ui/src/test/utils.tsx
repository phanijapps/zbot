import { ReactElement, ReactNode } from 'react';
import { render, RenderOptions } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrowserRouter } from 'react-router-dom';

// Wrapper component with providers
interface WrapperProps {
  children: ReactNode;
}

function AllProviders({ children }: WrapperProps) {
  return <BrowserRouter>{children}</BrowserRouter>;
}

// Custom render function that includes providers
function customRender(
  ui: ReactElement,
  options?: Omit<RenderOptions, 'wrapper'>
) {
  return {
    user: userEvent.setup(),
    ...render(ui, { wrapper: AllProviders, ...options }),
  };
}

// Re-export everything from testing-library
export * from '@testing-library/react';

// Override render with custom version
export { customRender as render };

// Export user event setup helper
export { userEvent };
