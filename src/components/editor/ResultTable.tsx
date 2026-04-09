interface Props {
  columns: string[];
  rows: (string | number | boolean | null)[][];
}

export default function ResultTable({ columns, rows }: Props) {
  console.log('ResultTable props:', { columns, rows });
  if (columns.length === 0) {
    return <div className="flex items-center justify-center h-16 text-xs text-[var(--text-muted)]">无结果</div>;
  }

  return (
    <table className="min-w-full text-xs border-collapse">
      <thead className="sticky top-0 z-10">
        <tr className="bg-[var(--bg-tertiary)]">
          <th className="px-2 py-1.5 text-right text-[var(--text-muted)] font-mono border-b border-r border-[var(--border)] w-10 select-none">
            #
          </th>
          {columns.map((col) => (
            <th key={col} className="px-3 py-1.5 text-left font-semibold text-[var(--text-secondary)] border-b border-r border-[var(--border)] whitespace-nowrap">
              {col}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, ri) => (
          <tr key={ri} className="hover:bg-brand-500/5 even:bg-[var(--bg-secondary)]/30 transition-colors">
            <td className="px-2 py-1 text-right text-[var(--text-muted)] font-mono border-r border-[var(--border)] select-none">
              {ri + 1}
            </td>
            {row.map((cell, ci) => (
              <td
                key={ci}
                className="px-3 py-1 border-r border-[var(--border)] max-w-xs overflow-hidden text-ellipsis whitespace-nowrap"
                title={cell !== null ? String(cell) : 'NULL'}
              >
                {cell === null ? (
                  <span className="text-[var(--text-muted)] italic font-mono text-[11px]">NULL</span>
                ) : typeof cell === 'number' ? (
                  <span className="text-purple-400 font-mono">{cell}</span>
                ) : typeof cell === 'boolean' ? (
                  <span className={cell ? 'text-green-400' : 'text-red-400'}>{cell ? 'true' : 'false'}</span>
                ) : (
                  <span className="text-[var(--text-primary)]">{String(cell)}</span>
                )}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
