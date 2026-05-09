import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { formatFileSize, formatJson, getArtifactIcon, CsvTable } from './artifact-utils';

describe('formatFileSize', () => {
  it('returns empty for undefined', () => expect(formatFileSize(undefined)).toBe(''));
  it('returns empty for 0', () => expect(formatFileSize(0)).toBe(''));
  it('formats bytes', () => expect(formatFileSize(500)).toBe('500 B'));
  it('formats KB', () => expect(formatFileSize(2048)).toBe('2.0 KB'));
  it('formats MB', () => expect(formatFileSize(1500000)).toBe('1.4 MB'));
});

describe('formatJson', () => {
  it('formats valid JSON', () => {
    expect(formatJson('{"a":1}')).toBe('{\n  "a": 1\n}');
  });
  it('returns invalid JSON as-is', () => {
    expect(formatJson('not json')).toBe('not json');
  });
});

// ─── getArtifactIcon — render each branch ────────────────────────────────────

function renderIcon(fileType?: string) {
  const el = getArtifactIcon(fileType);
  const { container } = render(<>{el}</>);
  return container;
}

describe('getArtifactIcon', () => {
  it('renders for md', () => { expect(renderIcon('md').firstChild).toBeTruthy(); });
  it('renders for txt', () => { expect(renderIcon('txt').firstChild).toBeTruthy(); });
  it('renders for docx', () => { expect(renderIcon('docx').firstChild).toBeTruthy(); });
  it('renders for rs', () => { expect(renderIcon('rs').firstChild).toBeTruthy(); });
  it('renders for py', () => { expect(renderIcon('py').firstChild).toBeTruthy(); });
  it('renders for js', () => { expect(renderIcon('js').firstChild).toBeTruthy(); });
  it('renders for ts', () => { expect(renderIcon('ts').firstChild).toBeTruthy(); });
  it('renders for tsx', () => { expect(renderIcon('tsx').firstChild).toBeTruthy(); });
  it('renders for jsx', () => { expect(renderIcon('jsx').firstChild).toBeTruthy(); });
  it('renders for csv', () => { expect(renderIcon('csv').firstChild).toBeTruthy(); });
  it('renders for json', () => { expect(renderIcon('json').firstChild).toBeTruthy(); });
  it('renders for xlsx', () => { expect(renderIcon('xlsx').firstChild).toBeTruthy(); });
  it('renders for html', () => { expect(renderIcon('html').firstChild).toBeTruthy(); });
  it('renders for htm', () => { expect(renderIcon('htm').firstChild).toBeTruthy(); });
  it('renders for png', () => { expect(renderIcon('png').firstChild).toBeTruthy(); });
  it('renders for jpg', () => { expect(renderIcon('jpg').firstChild).toBeTruthy(); });
  it('renders for jpeg', () => { expect(renderIcon('jpeg').firstChild).toBeTruthy(); });
  it('renders for gif', () => { expect(renderIcon('gif').firstChild).toBeTruthy(); });
  it('renders for svg', () => { expect(renderIcon('svg').firstChild).toBeTruthy(); });
  it('renders for mp4', () => { expect(renderIcon('mp4').firstChild).toBeTruthy(); });
  it('renders for webm', () => { expect(renderIcon('webm').firstChild).toBeTruthy(); });
  it('renders for mp3', () => { expect(renderIcon('mp3').firstChild).toBeTruthy(); });
  it('renders for wav', () => { expect(renderIcon('wav').firstChild).toBeTruthy(); });
  it('renders for pptx', () => { expect(renderIcon('pptx').firstChild).toBeTruthy(); });
  it('renders for pdf', () => { expect(renderIcon('pdf').firstChild).toBeTruthy(); });
  it('renders for unknown type', () => { expect(renderIcon('xyz').firstChild).toBeTruthy(); });
  it('renders for undefined', () => { expect(renderIcon(undefined).firstChild).toBeTruthy(); });
});

// ─── CsvTable ─────────────────────────────────────────────────────────────────

describe('CsvTable', () => {
  it('renders a table with headers and rows', () => {
    render(<CsvTable content={"Name,Age\nAlice,30\nBob,25"} />);
    expect(screen.getByRole('table')).toBeInTheDocument();
    expect(screen.getByText('Name')).toBeInTheDocument();
    expect(screen.getByText('Alice')).toBeInTheDocument();
    expect(screen.getByText('30')).toBeInTheDocument();
  });

  it('renders pre for empty content', () => {
    const { container } = render(<CsvTable content={''} />);
    expect(container.querySelector('pre')).toBeInTheDocument();
  });

  it('caps body rows at 100', () => {
    const rows = ['h1,h2', ...Array.from({ length: 110 }, (_, i) => `r${i},v${i}`)].join('\n');
    const { container } = render(<CsvTable content={rows} />);
    const trs = container.querySelectorAll('tbody tr');
    expect(trs.length).toBeLessThanOrEqual(100);
  });
});
