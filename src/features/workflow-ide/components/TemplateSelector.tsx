import React, { useState, useMemo } from 'react';
import { X, Plus, ArrowRight, GitBranch, Layers, Network, Workflow } from 'lucide-react';
import { WORKFLOW_TEMPLATES, layoutTemplateNodes, type WorkflowTemplate } from '../types/templates';
import { cn } from '@/core/utils/cn';

interface TemplateSelectorProps {
  onClose: () => void;
  onApplyTemplate: (template: WorkflowTemplate) => void;
}

export const TemplateSelector: React.FC<TemplateSelectorProps> = ({ onClose, onApplyTemplate }) => {
  const [selectedCategory, setSelectedCategory] = useState<'all' | WorkflowTemplate['category']>('all');

  const filteredTemplates = useMemo(() => {
    if (selectedCategory === 'all') return WORKFLOW_TEMPLATES;
    return WORKFLOW_TEMPLATES.filter((t) => t.category === selectedCategory);
  }, [selectedCategory]);

  const getTemplateIcon = (templateId: string) => {
    switch (templateId) {
      case 'pipeline':
        return <ArrowRight size={24} />;
      case 'swarm':
        return <Network size={24} />;
      case 'router':
        return <GitBranch size={24} />;
      case 'map-reduce':
        return <Layers size={24} />;
      case 'hierarchical':
        return <Workflow size={24} />;
      default:
        return <Plus size={24} />;
    }
  };

  const handleApplyTemplate = (template: WorkflowTemplate) => {
    const layoutedTemplate = layoutTemplateNodes(template);
    onApplyTemplate(layoutedTemplate);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-4xl max-h-[80vh] bg-gray-900 rounded-lg shadow-2xl border border-gray-700 overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-700 bg-gray-950">
          <div>
            <h2 className="text-xl font-semibold text-white">Workflow Templates</h2>
            <p className="text-sm text-gray-400 mt-1">
              Start with a pre-defined workflow pattern
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-2 text-gray-400 hover:text-white hover:bg-gray-800 rounded-md transition-colors"
          >
            <X size={20} />
          </button>
        </div>

        {/* Category Filter */}
        <div className="flex items-center gap-2 px-6 py-3 border-b border-gray-800 bg-gray-900">
          <span className="text-sm text-gray-400">Category:</span>
          <div className="flex gap-1">
            <button
              onClick={() => setSelectedCategory('all')}
              className={cn(
                'px-3 py-1.5 text-sm rounded-md transition-colors',
                selectedCategory === 'all'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
              )}
            >
              All
            </button>
            <button
              onClick={() => setSelectedCategory('basic')}
              className={cn(
                'px-3 py-1.5 text-sm rounded-md transition-colors',
                selectedCategory === 'basic'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
              )}
            >
              Basic
            </button>
            <button
              onClick={() => setSelectedCategory('advanced')}
              className={cn(
                'px-3 py-1.5 text-sm rounded-md transition-colors',
                selectedCategory === 'advanced'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
              )}
            >
              Advanced
            </button>
            <button
              onClick={() => setSelectedCategory('specialized')}
              className={cn(
                'px-3 py-1.5 text-sm rounded-md transition-colors',
                selectedCategory === 'specialized'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
              )}
            >
              Specialized
            </button>
          </div>
        </div>

        {/* Template Grid */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {filteredTemplates.map((template) => (
              <button
                key={template.id}
                onClick={() => handleApplyTemplate(template)}
                className="flex flex-col p-4 bg-gray-800 hover:bg-gray-750 border border-gray-700 hover:border-blue-500 rounded-lg transition-all text-left group"
              >
                {/* Icon and Name */}
                <div className="flex items-start gap-3 mb-2">
                  <div className="p-2 bg-blue-500/20 text-blue-400 rounded-lg group-hover:bg-blue-500/30 transition-colors">
                    {getTemplateIcon(template.id)}
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3 className="font-semibold text-white text-lg">
                      {template.name}
                    </h3>
                    <div className="flex items-center gap-2 mt-1">
                      <span className="text-xs px-2 py-0.5 bg-gray-700 text-gray-300 rounded">
                        {template.category}
                      </span>
                      <span className="text-xs text-gray-500">
                        {template.nodes.length} nodes
                      </span>
                    </div>
                  </div>
                </div>

                {/* Description */}
                <p className="text-sm text-gray-400 line-clamp-2">
                  {template.description}
                </p>

                {/* Visual preview */}
                <div className="mt-3 pt-3 border-t border-gray-700">
                  <div className="text-xs text-gray-500 mb-2">Preview:</div>
                  <div className="flex items-center gap-1">
                    <div className="w-8 h-8 bg-amber-500/20 border border-amber-500 rounded flex items-center justify-center">
                      <div className="w-2 h-2 bg-amber-500 rounded-full" />
                    </div>
                    <div className="text-gray-600">→</div>
                    {template.nodes.slice(1, 4).map((node, i) => (
                      <React.Fragment key={node.id}>
                        <div className="w-6 h-6 bg-purple-500/20 border border-purple-500 rounded flex items-center justify-center">
                          <div className="w-1.5 h-1.5 bg-purple-500 rounded-full" />
                        </div>
                        {i < Math.min(template.nodes.length - 2, 2) && (
                          <div className="text-gray-600">
                            {template.id === 'pipeline' ? '→' : '⋯'}
                          </div>
                        )}
                      </React.Fragment>
                    ))}
                    {template.nodes.length > 4 && (
                      <span className="text-xs text-gray-500">+</span>
                    )}
                  </div>
                </div>
              </button>
            ))}
          </div>
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-gray-700 bg-gray-950">
          <p className="text-sm text-gray-400">
            Click a template to apply it to your workflow. You can modify it afterwards.
          </p>
        </div>
      </div>
    </div>
  );
};
