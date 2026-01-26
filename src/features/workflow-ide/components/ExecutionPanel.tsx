// ============================================================================
// EXECUTION PANEL
// Side panel showing workflow execution timeline, output, and logs
// ============================================================================

import React, { useEffect, useRef } from 'react';
import { X, Play, Square, CheckCircle2, XCircle, Clock, ChevronDown, ChevronRight, Loader2 } from 'lucide-react';
import { useWorkflowStore } from '../stores/workflowStore';
import type { NodeExecutionStatus } from '../types/workflow';

interface ExecutionPanelProps {
  isOpen: boolean;
  onClose: () => void;
  isExecuting: boolean;
  onStop: () => void;
  onExecute: (message: string) => void;
  streamOutput: string;
  executionError: string | null;
}

interface TimelineEntry {
  nodeId: string;
  nodeName: string;
  status: NodeExecutionStatus;
  startedAt?: Date;
  completedAt?: Date;
  input?: string;
  output?: string;
}

export const ExecutionPanel: React.FC<ExecutionPanelProps> = ({
  isOpen,
  onClose,
  isExecuting,
  onStop,
  onExecute,
  streamOutput,
  executionError,
}) => {
  const { execution, nodes } = useWorkflowStore();
  const [inputMessage, setInputMessage] = React.useState('');
  const [activeTab, setActiveTab] = React.useState<'timeline' | 'output' | 'logs'>('timeline');
  const [expandedNodes, setExpandedNodes] = React.useState<Set<string>>(new Set());
  const outputRef = useRef<HTMLDivElement>(null);
  const logsRef = useRef<HTMLDivElement>(null);

  // Auto-scroll output
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [streamOutput]);

  // Auto-scroll logs
  useEffect(() => {
    if (logsRef.current) {
      logsRef.current.scrollTop = logsRef.current.scrollHeight;
    }
  }, [execution.logs]);

  // Build timeline from execution state
  const timeline: TimelineEntry[] = React.useMemo(() => {
    const entries: TimelineEntry[] = [];

    // Get nodes in execution order (start first, then subagents, then end)
    const orderedNodes = [...nodes].sort((a, b) => {
      if (a.type === 'start') return -1;
      if (b.type === 'start') return 1;
      if (a.type === 'end') return 1;
      if (b.type === 'end') return -1;
      return 0;
    });

    for (const node of orderedNodes) {
      const state = execution.nodeStates[node.id];
      entries.push({
        nodeId: node.id,
        nodeName: (node.data?.label as string) || (node.data?.displayName as string) || node.id,
        status: state?.status || 'idle',
        startedAt: state?.startedAt,
        completedAt: state?.completedAt,
      });
    }

    return entries;
  }, [nodes, execution.nodeStates]);

  const toggleNodeExpanded = (nodeId: string) => {
    setExpandedNodes(prev => {
      const next = new Set(prev);
      if (next.has(nodeId)) {
        next.delete(nodeId);
      } else {
        next.add(nodeId);
      }
      return next;
    });
  };

  const getStatusIcon = (status: NodeExecutionStatus) => {
    switch (status) {
      case 'running':
        return <Loader2 size={16} className="animate-spin text-blue-400" />;
      case 'completed':
        return <CheckCircle2 size={16} className="text-green-400" />;
      case 'failed':
        return <XCircle size={16} className="text-red-400" />;
      case 'pending':
        return <Clock size={16} className="text-gray-500" />;
      default:
        return <div className="w-4 h-4 rounded-full border-2 border-gray-600" />;
    }
  };

  const getStatusBgColor = (status: NodeExecutionStatus) => {
    switch (status) {
      case 'running':
        return 'bg-blue-500/10 border-blue-500/30';
      case 'completed':
        return 'bg-green-500/10 border-green-500/30';
      case 'failed':
        return 'bg-red-500/10 border-red-500/30';
      default:
        return 'bg-gray-800/50 border-gray-700';
    }
  };

  const formatDuration = (start?: Date, end?: Date) => {
    if (!start) return '';
    const endTime = end || new Date();
    const duration = (endTime.getTime() - start.getTime()) / 1000;
    return `${duration.toFixed(1)}s`;
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (inputMessage.trim() && !isExecuting) {
      onExecute(inputMessage);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="w-96 h-full bg-gray-900 border-l border-gray-700 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-700">
        <div className="flex items-center gap-3">
          <h3 className="font-semibold text-white">Execution</h3>
          {isExecuting && (
            <span className="flex items-center gap-1.5 text-xs text-blue-400 bg-blue-500/10 px-2 py-1 rounded-full">
              <Loader2 size={12} className="animate-spin" />
              Running
            </span>
          )}
          {!isExecuting && execution.logs.length > 0 && !executionError && (
            <span className="flex items-center gap-1.5 text-xs text-green-400 bg-green-500/10 px-2 py-1 rounded-full">
              <CheckCircle2 size={12} />
              Completed
            </span>
          )}
          {executionError && (
            <span className="flex items-center gap-1.5 text-xs text-red-400 bg-red-500/10 px-2 py-1 rounded-full">
              <XCircle size={12} />
              Failed
            </span>
          )}
        </div>
        <button
          onClick={onClose}
          className="p-1 hover:bg-gray-800 rounded transition-colors"
        >
          <X size={18} className="text-gray-400" />
        </button>
      </div>

      {/* Input Form */}
      <form onSubmit={handleSubmit} className="p-4 border-b border-gray-700">
        <div className="flex gap-2">
          <input
            type="text"
            value={inputMessage}
            onChange={(e) => setInputMessage(e.target.value)}
            placeholder="Enter your message..."
            className="flex-1 px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-sm text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
            disabled={isExecuting}
          />
          {isExecuting ? (
            <button
              type="button"
              onClick={onStop}
              className="px-3 py-2 bg-red-600 hover:bg-red-700 rounded-lg transition-colors"
            >
              <Square size={18} className="text-white" />
            </button>
          ) : (
            <button
              type="submit"
              disabled={!inputMessage.trim()}
              className="px-3 py-2 bg-green-600 hover:bg-green-700 disabled:bg-gray-700 disabled:text-gray-500 rounded-lg transition-colors"
            >
              <Play size={18} className="text-white" />
            </button>
          )}
        </div>
      </form>

      {/* Tabs */}
      <div className="flex border-b border-gray-700">
        {(['timeline', 'output', 'logs'] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${
              activeTab === tab
                ? 'text-blue-400 border-b-2 border-blue-400 bg-gray-800/50'
                : 'text-gray-400 hover:text-gray-300 hover:bg-gray-800/30'
            }`}
          >
            {tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {/* Timeline Tab */}
        {activeTab === 'timeline' && (
          <div className="h-full overflow-y-auto p-4 space-y-2">
            {timeline.length === 0 ? (
              <p className="text-gray-500 text-sm text-center py-8">
                Run the workflow to see execution timeline
              </p>
            ) : (
              timeline.map((entry) => (
                <div
                  key={entry.nodeId}
                  className={`border rounded-lg overflow-hidden transition-all ${getStatusBgColor(entry.status)}`}
                >
                  <button
                    onClick={() => toggleNodeExpanded(entry.nodeId)}
                    className="w-full flex items-center gap-3 p-3 text-left hover:bg-white/5 transition-colors"
                  >
                    {expandedNodes.has(entry.nodeId) ? (
                      <ChevronDown size={14} className="text-gray-400" />
                    ) : (
                      <ChevronRight size={14} className="text-gray-400" />
                    )}
                    {getStatusIcon(entry.status)}
                    <span className="flex-1 text-sm font-medium text-white truncate">
                      {entry.nodeName}
                    </span>
                    {entry.startedAt && (
                      <span className="text-xs text-gray-500">
                        {formatDuration(entry.startedAt, entry.completedAt)}
                      </span>
                    )}
                  </button>

                  {expandedNodes.has(entry.nodeId) && (
                    <div className="px-4 pb-3 pt-1 border-t border-gray-700/50">
                      <div className="text-xs space-y-2">
                        {entry.status !== 'idle' && (
                          <>
                            <div>
                              <span className="text-gray-500">Status:</span>{' '}
                              <span className="text-gray-300">{entry.status}</span>
                            </div>
                            {entry.startedAt && (
                              <div>
                                <span className="text-gray-500">Started:</span>{' '}
                                <span className="text-gray-300">
                                  {entry.startedAt.toLocaleTimeString()}
                                </span>
                              </div>
                            )}
                          </>
                        )}
                        {/* Show relevant logs for this node */}
                        {execution.logs
                          .filter(log => log.nodeId === entry.nodeId)
                          .slice(-3)
                          .map(log => (
                            <div key={log.id} className="text-gray-400 truncate">
                              {log.message}
                            </div>
                          ))}
                      </div>
                    </div>
                  )}
                </div>
              ))
            )}
          </div>
        )}

        {/* Output Tab */}
        {activeTab === 'output' && (
          <div
            ref={outputRef}
            className="h-full overflow-y-auto p-4 font-mono text-sm text-gray-300 whitespace-pre-wrap"
          >
            {streamOutput || (
              <span className="text-gray-500">Output will appear here...</span>
            )}
          </div>
        )}

        {/* Logs Tab */}
        {activeTab === 'logs' && (
          <div ref={logsRef} className="h-full overflow-y-auto p-4 space-y-1">
            {execution.logs.length === 0 ? (
              <p className="text-gray-500 text-sm text-center py-8">
                No logs yet
              </p>
            ) : (
              execution.logs.map((log) => (
                <div
                  key={log.id}
                  className={`text-xs font-mono py-1 px-2 rounded ${
                    log.level === 'error'
                      ? 'bg-red-500/10 text-red-400'
                      : log.level === 'warn'
                      ? 'bg-yellow-500/10 text-yellow-400'
                      : 'text-gray-400'
                  }`}
                >
                  <span className="text-gray-600">
                    {log.timestamp.toLocaleTimeString()}
                  </span>{' '}
                  {log.nodeId && (
                    <span className="text-blue-400">[{log.nodeId}]</span>
                  )}{' '}
                  {log.message}
                </div>
              ))
            )}
          </div>
        )}
      </div>

      {/* Error Display */}
      {executionError && (
        <div className="p-4 border-t border-red-800 bg-red-900/20">
          <p className="text-sm text-red-400">{executionError}</p>
        </div>
      )}
    </div>
  );
};
