import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@/test/utils';
import { ThinkingBlock } from './ThinkingBlock';

describe('ThinkingBlock', () => {
  it('renders collapsed by default', () => {
    render(<ThinkingBlock content="thinking content" />);
    expect(screen.getByText('Thinking')).toBeDefined();
    expect(screen.queryByText('thinking content')).toBeNull();
  });

  it('expands on click', () => {
    render(<ThinkingBlock content="thinking content" />);
    fireEvent.click(screen.getByText('Thinking'));
    expect(screen.getByText('thinking content')).toBeDefined();
  });
});
