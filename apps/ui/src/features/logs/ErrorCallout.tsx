interface ErrorCalloutProps {
  timestamp: string;
  message: string;
}

export function ErrorCallout({ timestamp, message }: ErrorCalloutProps) {
  const time = new Date(timestamp).toLocaleTimeString('en-US', { hour12: false });
  return (
    <div className="error-callout">
      <span className="error-callout__time">{time}</span>
      <span className="error-callout__message">{message}</span>
    </div>
  );
}
