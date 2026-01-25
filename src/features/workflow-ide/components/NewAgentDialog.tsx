// ============================================================================
// NEW AGENT DIALOG
// Dialog for creating a new agent with optional workflow template
// ============================================================================

import React, { useState } from 'react';
import { X, Plus, Sparkles, Loader2 } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { TemplateSelector } from './TemplateSelector';
import type { WorkflowTemplate } from '../types/templates';
import * as agentService from '@/services/agent';
import * as workflowService from '@/services/workflow';

type Step = 'name' | 'template' | 'creating';

interface NewAgentDialogProps {
  onClose: () => void;
}

export const NewAgentDialog: React.FC<NewAgentDialogProps> = ({ onClose }) => {
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>('name');
  const [agentName, setAgentName] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  // Validate and proceed to template selection
  const handleNameSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    // Validate name
    const validation = agentService.validateAgentName(agentName);
    if (!validation.valid) {
      setNameError(validation.error || 'Invalid name');
      return;
    }

    setNameError(null);
    setStep('template');
  };

  // Handle template selection
  const handleTemplateSelected = async (template: WorkflowTemplate) => {
    setIsCreating(true);
    setStep('creating');

    try {
      // 1. Create the agent
      const newAgent = await agentService.createAgent({
        name: agentName,
        displayName: agentName,
        description: '',
        providerId: '',
        model: '',
        temperature: 0.7,
        maxTokens: 2000,
        instructions: '',
        mcps: [],
        skills: [],
      });

      // 2. Save the workflow template to the agent
      await workflowService.saveOrchestratorStructure(newAgent.id, {
        nodes: template.nodes,
        edges: template.edges,
        orchestrator: template.orchestrator,
      });

      // 3. Navigate to Workflow IDE
      navigate(`/workflow/${newAgent.id}`, {
        state: { from: '/', isNewAgent: true },
      });
    } catch (error) {
      console.error('Failed to create agent:', error);
      setNameError(error instanceof Error ? error.message : 'Failed to create agent');
      setStep('name');
      setIsCreating(false);
    }
  };

  // Handle blank workflow (no template)
  const handleBlankWorkflow = async () => {
    setIsCreating(true);
    setStep('creating');

    try {
      // 1. Create the agent
      const newAgent = await agentService.createAgent({
        name: agentName,
        displayName: agentName,
        description: '',
        providerId: '',
        model: '',
        temperature: 0.7,
        maxTokens: 2000,
        instructions: '',
        mcps: [],
        skills: [],
      });

      // 2. Navigate to Workflow IDE with empty workflow
      navigate(`/workflow/${newAgent.id}`, {
        state: { from: '/', isNewAgent: true },
      });
    } catch (error) {
      console.error('Failed to create agent:', error);
      setNameError(error instanceof Error ? error.message : 'Failed to create agent');
      setStep('name');
      setIsCreating(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-full max-w-md bg-gray-900 rounded-lg shadow-2xl border border-gray-700 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-700 bg-gray-950">
          <div className="flex items-center gap-2">
            <Sparkles className="text-purple-400" size={20} />
            <h2 className="text-lg font-semibold text-white">
              {step === 'name' ? 'Create New Agent' : step === 'template' ? 'Choose Template' : 'Creating...'}
            </h2>
          </div>
          <button
            onClick={onClose}
            disabled={step === 'creating'}
            className="p-2 text-gray-400 hover:text-white hover:bg-gray-800 rounded-md transition-colors disabled:opacity-50"
          >
            <X size={18} />
          </button>
        </div>

        {/* Content */}
        <div className="p-6">
          {step === 'name' && (
            <form onSubmit={handleNameSubmit} className="space-y-4">
              <div>
                <label htmlFor="agentName" className="block text-sm font-medium text-gray-300 mb-2">
                  Agent Name
                </label>
                <input
                  id="agentName"
                  type="text"
                  value={agentName}
                  onChange={(e) => {
                    setAgentName(e.target.value);
                    setNameError(null);
                  }}
                  placeholder="e.g., my-awesome-agent"
                  className="w-full px-4 py-2 bg-gray-800 border border-gray-700 rounded-lg text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                  autoFocus
                  disabled={isCreating}
                />
                {nameError && (
                  <p className="mt-2 text-sm text-red-400">{nameError}</p>
                )}
                <p className="mt-2 text-xs text-gray-500">
                  Lowercase letters, numbers, and hyphens only
                </p>
              </div>

              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={onClose}
                  disabled={isCreating}
                  className="px-4 py-2 text-sm text-gray-300 hover:text-white hover:bg-gray-800 rounded-lg transition-colors disabled:opacity-50"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={!agentName || isCreating}
                  className="px-4 py-2 text-sm bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors disabled:bg-gray-700 disabled:text-gray-500 disabled:cursor-not-allowed flex items-center gap-2"
                >
                  Continue
                </button>
              </div>
            </form>
          )}

          {step === 'template' && (
            <div className="space-y-4">
              <p className="text-sm text-gray-400">
                Start with a template or create a blank workflow
              </p>

              <div className="grid grid-cols-1 gap-3">
                {/* Blank workflow option */}
                <button
                  onClick={handleBlankWorkflow}
                  disabled={isCreating}
                  className="flex items-center gap-3 p-4 bg-gray-800 hover:bg-gray-750 border border-gray-700 hover:border-blue-500 rounded-lg transition-all text-left group"
                >
                  <div className="p-2 bg-gray-700 text-gray-400 group-hover:bg-gray-600 rounded-lg transition-colors">
                    <Plus size={20} />
                  </div>
                  <div>
                    <h3 className="font-semibold text-white">Blank Workflow</h3>
                    <p className="text-sm text-gray-400">Start from scratch</p>
                  </div>
                </button>

                {/* Templates option */}
                <button
                  onClick={() => {
                    // Show full template selector
                    const templateSelector = document.querySelector('[data-template-selector]') as HTMLElement;
                    if (templateSelector) {
                      templateSelector.style.display = 'flex';
                    }
                  }}
                  disabled={isCreating}
                  className="flex items-center gap-3 p-4 bg-gray-800 hover:bg-gray-750 border border-gray-700 hover:border-purple-500 rounded-lg transition-all text-left group"
                >
                  <div className="p-2 bg-purple-500/20 text-purple-400 group-hover:bg-purple-500/30 rounded-lg transition-colors">
                    <Sparkles size={20} />
                  </div>
                  <div>
                    <h3 className="font-semibold text-white">Use Template</h3>
                    <p className="text-sm text-gray-400">Choose a pre-built workflow pattern</p>
                  </div>
                </button>
              </div>

              <div className="flex justify-end pt-2">
                <button
                  onClick={() => setStep('name')}
                  disabled={isCreating}
                  className="px-4 py-2 text-sm text-gray-300 hover:text-white hover:bg-gray-800 rounded-lg transition-colors disabled:opacity-50"
                >
                  Back
                </button>
              </div>
            </div>
          )}

          {step === 'creating' && (
            <div className="flex flex-col items-center justify-center py-8">
              <Loader2 size={32} className="text-blue-500 animate-spin mb-4" />
              <p className="text-white font-medium">Creating agent...</p>
              <p className="text-sm text-gray-400 mt-1">{agentName}</p>
            </div>
          )}
        </div>
      </div>

      {/* Template Selector Overlay (hidden by default) */}
      {step === 'template' && (
        <div data-template-selector className="hidden" style={{ display: 'none' }}>
          <TemplateSelector
            onClose={() => {
              const selector = document.querySelector('[data-template-selector]') as HTMLElement;
              if (selector) selector.style.display = 'none';
            }}
            onApplyTemplate={handleTemplateSelected}
          />
        </div>
      )}
    </div>
  );
};
