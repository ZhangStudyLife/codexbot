interface StatusBadgeProps {
  ready: boolean;
  readyLabel: string;
  pendingLabel: string;
}

export function StatusBadge({ ready, readyLabel, pendingLabel }: StatusBadgeProps) {
  return (
    <span className={`status-badge ${ready ? "is-ready" : "is-pending"}`}>
      <span aria-hidden="true" />
      {ready ? readyLabel : pendingLabel}
    </span>
  );
}
