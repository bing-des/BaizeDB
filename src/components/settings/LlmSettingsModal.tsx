import { useState, useEffect } from 'react';
import { X, Bot, Check, AlertCircle, Eye, EyeOff } from 'lucide-react';
import { llmApi } from '../../utils/api';
import type { LlmConfig } from '../../types';

interface LlmSettingsModalProps {
  onClose: () => void;
}

export default function LlmSettingsModal({ onClose }: LlmSettingsModalProps) {
  const [config, setConfig] = useState<LlmConfig>({
    api_key: '',
    api_url: 'https://api.openai.com/v1/chat/completions',
    model: 'gpt-3.5-turbo',
    enabled: false,
  });
  const [showApiKey, setShowApiKey] = useState(false);
  const [loading, setLoading] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [saveSuccess, setSaveSuccess] = useState(false);

  // 加载配置
  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const response = await llmApi.getConfig();
      if (response.config) {
        setConfig(response.config);
      }
    } catch (error) {
      console.error('加载 LLM 配置失败:', error);
    }
  };

  const handleSave = async () => {
    setLoading(true);
    setTestResult(null);
    try {
      await llmApi.saveConfig(config);
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2000);
    } catch (error) {
      console.error('保存 LLM 配置失败:', error);
      setTestResult({ success: false, message: '保存失败: ' + String(error) });
    } finally {
      setLoading(false);
    }
  };

  const handleTest = async () => {
    if (!config.api_key) {
      setTestResult({ success: false, message: '请先输入 API Key' });
      return;
    }

    setTesting(true);
    setTestResult(null);
    try {
      const result = await llmApi.testConfig(config);
      setTestResult({ success: true, message: result });
    } catch (error) {
      setTestResult({ success: false, message: '测试失败: ' + String(error) });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div className="w-[480px] max-w-[90vw] bg-[var(--bg-secondary)] rounded-lg shadow-2xl border border-[var(--border)] overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border)] bg-[var(--bg-tertiary)]">
          <div className="flex items-center gap-2">
            <Bot size={18} className="text-brand-400" />
            <h3 className="font-semibold text-sm">AI 设置</h3>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded hover:bg-[var(--bg-primary)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-4">
          {/* 启用开关 */}
          <div className="flex items-center justify-between p-3 bg-[var(--bg-tertiary)] rounded-lg">
            <div>
              <div className="text-sm font-medium">启用 AI 分析</div>
              <div className="text-xs text-[var(--text-muted)] mt-0.5">
                开启后可自动分析表关系
              </div>
            </div>
            <button
              onClick={() => setConfig(c => ({ ...c, enabled: !c.enabled }))}
              className={`relative w-11 h-6 rounded-full transition-colors ${
                config.enabled ? 'bg-brand-500' : 'bg-[var(--border)]'
              }`}
            >
              <span
                className={`absolute top-1 left-1 w-4 h-4 bg-white rounded-full transition-transform ${
                  config.enabled ? 'translate-x-5' : 'translate-x-0'
                }`}
              />
            </button>
          </div>

          {/* API URL */}
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-[var(--text-secondary)]">
              API 地址
            </label>
            <input
              type="text"
              value={config.api_url}
              onChange={e => setConfig(c => ({ ...c, api_url: e.target.value }))}
              placeholder="https://api.openai.com/v1/chat/completions"
              className="w-full px-3 py-2 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-500 focus:ring-1 focus:ring-brand-500/20 transition-all"
            />
            <div className="text-[10px] text-[var(--text-muted)]">
              支持 OpenAI 格式，如：OpenAI、Azure、Claude 等
            </div>
          </div>

          {/* Model */}
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-[var(--text-secondary)]">
              模型
            </label>
            <input
              type="text"
              value={config.model}
              onChange={e => setConfig(c => ({ ...c, model: e.target.value }))}
              placeholder="gpt-3.5-turbo"
              className="w-full px-3 py-2 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-500 focus:ring-1 focus:ring-brand-500/20 transition-all"
            />
          </div>

          {/* API Key */}
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-[var(--text-secondary)]">
              API Key
            </label>
            <div className="relative">
              <input
                type={showApiKey ? 'text' : 'password'}
                value={config.api_key}
                onChange={e => setConfig(c => ({ ...c, api_key: e.target.value }))}
                placeholder="sk-..."
                className="w-full px-3 py-2 pr-10 text-sm bg-[var(--bg-primary)] border border-[var(--border)] rounded-md focus:outline-none focus:border-brand-500 focus:ring-1 focus:ring-brand-500/20 transition-all"
              />
              <button
                onClick={() => setShowApiKey(!showApiKey)}
                className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-[var(--text-muted)] hover:text-[var(--text-secondary)]"
              >
                {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
              </button>
            </div>
            <div className="text-[10px] text-[var(--text-muted)]">
              API Key 将加密存储在本地 SQLite 中
            </div>
          </div>

          {/* Test Result */}
          {testResult && (
            <div
              className={`flex items-start gap-2 p-3 rounded-lg text-xs ${
                testResult.success
                  ? 'bg-green-500/10 border border-green-500/30 text-green-400'
                  : 'bg-red-500/10 border border-red-500/30 text-red-400'
              }`}
            >
              {testResult.success ? <Check size={14} /> : <AlertCircle size={14} />}
              <span className="flex-1">{testResult.message}</span>
            </div>
          )}

          {/* Save Success */}
          {saveSuccess && (
            <div className="flex items-center gap-2 p-3 rounded-lg text-xs bg-green-500/10 border border-green-500/30 text-green-400">
              <Check size={14} />
              <span>保存成功</span>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-[var(--border)] bg-[var(--bg-tertiary)]">
          <button
            onClick={handleTest}
            disabled={testing || !config.api_key}
            className="px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] bg-[var(--bg-primary)] border border-[var(--border)] rounded-md hover:bg-[var(--bg-secondary)] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {testing ? '测试中...' : '测试连接'}
          </button>
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
          >
            取消
          </button>
          <button
            onClick={handleSave}
            disabled={loading}
            className="px-3 py-1.5 text-xs font-medium text-white bg-brand-500 hover:bg-brand-600 rounded-md disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {loading ? '保存中...' : '保存'}
          </button>
        </div>
      </div>
    </div>
  );
}
