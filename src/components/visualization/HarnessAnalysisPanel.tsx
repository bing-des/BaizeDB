import { useState, useEffect, useCallback, useRef } from 'react';
import { 
  Loader2, X, ChevronRight, CheckCircle, Clock, 
  Database, Search, Eye, Sparkles, AlertTriangle, ArrowRight, 
  TableIcon, Check
} from 'lucide-react';
import { harnessApi } from '../../utils/api';
import { useThemeStore } from '../../store';
import type { 
  HarnessSessionInfo, 
  HarnessAnalysisStep, 
  HarnessCandidatesResponse,
  HarnessRelationCandidate,
  TableRelationAnalysis,
  AnalysisStage 
} from '../../types';
import { getStageName, getAnalyzingTable } from '../../types';

interface HarnessAnalysisPanelProps {
  connectionId: string;
  database: string;
  schema?: string;
  onClose: () => void;
  onRelationsFound: (relations: TableRelationAnalysis[]) => void;
}

export default function HarnessAnalysisPanel({
  connectionId,
  database,
  schema,
  onClose,
  onRelationsFound,
}: HarnessAnalysisPanelProps) {
  const { theme } = useThemeStore();
  const [session, setSession] = useState<HarnessSessionInfo | null>(null);
  const [steps, setSteps] = useState<HarnessAnalysisStep[]>([]);
  const [candidates, setCandidates] = useState<HarnessCandidatesResponse | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [initialized, setInitialized] = useState(false);
  const stepsEndRef = useRef<HTMLDivElement>(null);
  const intervalRef = useRef<number | null>(null);

  const isDark = theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  // 初始化会话
  const initSession = useCallback(async () => {
    try {
      setError(null);
      const sessionInfo = await harnessApi.startAnalysis({
        connection_id: connectionId,
        database,
        schema,
      });
      setSession(sessionInfo);
      setInitialized(true);
      
      // 如果已完成，直接获取结果
      if (sessionInfo.is_complete) {
        await loadResults(sessionInfo.id);
      }
    } catch (e) {
      setError(String(e));
    }
  }, [connectionId, database, schema]);

  // 加载结果
  const loadResults = async (sessionId: string) => {
    try {
      const [stepsData, candidatesData] = await Promise.all([
        harnessApi.getSteps(sessionId),
        harnessApi.getCandidates(sessionId),
      ]);
      setSteps(stepsData);
      setCandidates(candidatesData);
    } catch (e) {
      console.error('加载结果失败:', e);
    }
  };

  // 运行一轮分析
  const runAnalysisTurn = useCallback(async () => {
    if (!session || running) return;
    
    setRunning(true);
    setError(null);
    
    try {
      const response = await harnessApi.runTurn(session.id);
      
      // 更新会话信息
      const updatedSession = await harnessApi.getSessionInfo(session.id);
      setSession(updatedSession);
      
      // 添加新步骤
      if (response.new_step) {
        setSteps(prev => [...prev, response.new_step!]);
      }
      
      // 如果完成，加载结果
      if (response.is_complete) {
        await loadResults(session.id);
        if (response.relations.length > 0) {
          onRelationsFound(response.relations);
        }
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }, [session, running, onRelationsFound]);

  // 自动运行（每 3 秒执行一轮）
  const startAutoRun = useCallback(() => {
    if (intervalRef.current) return;
    
    const runNext = async () => {
      const currentSession = await harnessApi.getSessionInfo(session!.id);
      setSession(currentSession);
      
      if (currentSession.is_complete) {
        stopAutoRun();
        await loadResults(currentSession.id);
        if (currentSession.relations_count > 0) {
          // 获取完整关系
          const candidatesData = await harnessApi.getCandidates(currentSession.id);
          const relations: TableRelationAnalysis[] = candidatesData.candidates
            .filter(c => c.verified)
            .map(c => ({
              source_table: c.source_table,
              source_column: c.source_column,
              target_table: c.target_table,
              target_column: c.target_column,
              relation_type: c.verification_method || 'detected',
              confidence: c.confidence,
              reason: c.reason,
            }));
          onRelationsFound(relations);
        }
        return;
      }
      
      await runAnalysisTurn();
    };
    
    runNext();
    intervalRef.current = window.setTimeout(function tick() {
      runNext().then(() => {
        if (intervalRef.current) {
          intervalRef.current = window.setTimeout(tick, 2000);
        }
      });
    }, 2000);
  }, [session, runAnalysisTurn, onRelationsFound]);

  const stopAutoRun = useCallback(() => {
    if (intervalRef.current) {
      clearTimeout(intervalRef.current);
      intervalRef.current = null;
    }
  }, []);

  // 初始化
  useEffect(() => {
    initSession();
    
    return () => {
      stopAutoRun();
    };
  }, [initSession, stopAutoRun]);

  // 自动滚动到最新步骤
  useEffect(() => {
    stepsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [steps]);

  // 获取步骤图标
  const getStepIcon = (step: HarnessAnalysisStep) => {
    if (step.step_type === 'tool_call') {
      return <Search size={14} className="text-blue-400" />;
    }
    if (step.step_type === 'tool_result') {
      return <Eye size={14} className="text-green-400" />;
    }
    return <Sparkles size={14} className="text-purple-400" />;
  };

  return (
    <div className={`h-full flex flex-col ${isDark ? 'bg-[#0f172a]' : 'bg-white'}`}>
      {/* Header */}
      <div className={`flex items-center justify-between px-4 py-3 border-b ${isDark ? 'border-[#334155] bg-[#1e293b]' : 'border-gray-200 bg-gray-50'}`}>
        <div className="flex items-center gap-2">
          <Sparkles size={18} className="text-purple-400" />
          <span className="font-medium text-sm">Harness AI 分析</span>
          <span className={`text-xs px-2 py-0.5 rounded-full ${isDark ? 'bg-purple-500/20 text-purple-400' : 'bg-purple-100 text-purple-600'}`}>
            {database}{schema ? `.${schema}` : ''}
          </span>
        </div>
        <button
          onClick={onClose}
          className={`p-1.5 rounded hover:bg-white/10 ${isDark ? 'text-slate-400' : 'text-gray-500'}`}
        >
          <X size={16} />
        </button>
      </div>

      {/* 阶段进度 */}
      {session && !session.is_complete && (
        <div className={`px-4 py-3 border-b ${isDark ? 'border-[#334155]' : 'border-gray-200'}`}>
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <Database size={14} className="text-blue-400" />
              <span className="text-xs font-medium">
                {getStageName(session.current_stage)}
              </span>
            </div>
            <span className={`text-xs ${isDark ? 'text-slate-500' : 'text-gray-400'}`}>
              {session.tables_analyzed}/{session.tables_total} 表
            </span>
          </div>
          
          {/* 当前分析表名 */}
          {getAnalyzingTable(session.current_stage) && (
            <div className={`mb-2 px-2 py-1 rounded text-xs font-mono ${
              isDark ? 'bg-purple-500/20 text-purple-400' : 'bg-purple-100 text-purple-600'
            }`}>
              正在分析: {getAnalyzingTable(session.current_stage)}
            </div>
          )}
          
          {/* 进度条 */}
          <div className={`h-1.5 rounded-full overflow-hidden ${isDark ? 'bg-[#334155]' : 'bg-gray-200'}`}>
            <div 
              className="h-full bg-gradient-to-r from-blue-500 to-purple-500 transition-all duration-500"
              style={{ width: `${(session.progress * 100).toFixed(1)}%` }}
            />
          </div>
        </div>
      )}

      {/* 统计信息 */}
      {candidates && (
        <div className={`grid grid-cols-2 gap-2 px-4 py-2 border-b ${isDark ? 'border-[#334155]' : 'border-gray-200'}`}>
          <div className={`text-center p-2 rounded ${isDark ? 'bg-[#1e293b]' : 'bg-gray-50'}`}>
            <div className={`text-lg font-bold ${isDark ? 'text-slate-300' : 'text-gray-700'}`}>
              {candidates.summary.total}
            </div>
            <div className={`text-[10px] ${isDark ? 'text-slate-500' : 'text-gray-400'}`}>候选关系</div>
          </div>
          <div className={`text-center p-2 rounded ${isDark ? 'bg-[#1e293b]' : 'bg-gray-50'}`}>
            <div className="text-lg font-bold text-purple-400">
              {Math.round(candidates.summary.avg_confidence * 100)}%
            </div>
            <div className={`text-[10px] ${isDark ? 'text-slate-500' : 'text-gray-400'}`}>平均置信</div>
          </div>
        </div>
      )}

      {/* 分析步骤 */}
      <div className={`flex-1 overflow-y-auto px-4 py-3 space-y-2 ${isDark ? 'bg-[#0f172a]' : 'bg-white'}`}>
        {steps.map((step, index) => (
          <div 
            key={index}
            className={`flex gap-2 p-2 rounded ${isDark ? 'bg-[#1e293b]/50' : 'bg-gray-50'}`}
          >
            <div className="flex-shrink-0 mt-0.5">
              {getStepIcon(step)}
            </div>
            <div className="flex-1 min-w-0">
              {step.tool_name && (
                <div className={`text-[10px] font-mono mb-0.5 ${isDark ? 'text-blue-400' : 'text-blue-600'}`}>
                  {step.tool_name}
                </div>
              )}
              <div className={`text-xs leading-relaxed ${isDark ? 'text-slate-300' : 'text-gray-700'}`}>
                {step.content}
              </div>
            </div>
          </div>
        ))}
        
        {running && (
          <div className={`flex items-center gap-2 p-2 rounded ${isDark ? 'bg-[#1e293b]/50' : 'bg-gray-50'}`}>
            <Loader2 size={14} className="animate-spin text-purple-400" />
            <span className={`text-xs ${isDark ? 'text-slate-400' : 'text-gray-500'}`}>
              AI 分析中...
            </span>
          </div>
        )}
        
        <div ref={stepsEndRef} />
      </div>

      {/* 候选关系列表 */}
      {candidates && candidates.candidates.length > 0 && (
        <div className={`border-t ${isDark ? 'border-[#334155]' : 'border-gray-200'}`}>
          <div className={`px-4 py-2 text-xs font-medium ${isDark ? 'text-slate-400 bg-[#1e293b]' : 'text-gray-500 bg-gray-50'}`}>
            候选关系 ({candidates.candidates.length})
          </div>
          <div className="max-h-48 overflow-y-auto px-4 pb-3 space-y-1.5">
            {candidates.candidates.slice(0, 20).map((c, i) => (
              <CandidateRow key={i} candidate={c} />
            ))}
            {candidates.candidates.length > 20 && (
              <div className={`text-[10px] text-center py-1 ${isDark ? 'text-slate-500' : 'text-gray-400'}`}>
                还有 {candidates.candidates.length - 20} 个候选关系...
              </div>
            )}
          </div>
        </div>
      )}

      {/* 错误信息 */}
      {error && (
        <div className={`px-4 py-2 border-t ${isDark ? 'border-red-500/30 bg-red-500/10' : 'border-red-200 bg-red-50'}`}>
          <div className="flex items-start gap-2 text-red-400 text-xs">
            <AlertTriangle size={14} className="flex-shrink-0 mt-0.5" />
            <span>{error}</span>
          </div>
        </div>
      )}

      {/* 操作按钮 */}
      <div className={`flex items-center justify-between gap-2 px-4 py-3 border-t ${isDark ? 'border-[#334155] bg-[#1e293b]' : 'border-gray-200 bg-gray-50'}`}>
        {!initialized ? (
          <div className="flex items-center gap-2 text-xs text-slate-500">
            <Loader2 size={14} className="animate-spin" />
            初始化中...
          </div>
        ) : session?.is_complete ? (
          <div className="flex items-center gap-2 text-xs text-green-400">
            <CheckCircle size={14} />
            分析完成
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <button
              onClick={runAnalysisTurn}
              disabled={running}
              className={`px-3 py-1.5 text-xs font-medium rounded transition-colors ${
                isDark 
                  ? 'bg-purple-600 hover:bg-purple-500 text-white disabled:bg-purple-800' 
                  : 'bg-purple-600 hover:bg-purple-500 text-white disabled:bg-purple-300'
              } disabled:cursor-not-allowed`}
            >
              {running ? (
                <>
                  <Loader2 size={12} className="inline animate-spin mr-1" />
                  分析中...
                </>
              ) : (
                <>
                  <ChevronRight size={12} className="inline mr-1" />
                  继续分析
                </>
              )}
            </button>
            
            {intervalRef.current ? (
              <button
                onClick={stopAutoRun}
                className={`px-3 py-1.5 text-xs font-medium rounded transition-colors ${
                  isDark 
                    ? 'bg-slate-700 hover:bg-slate-600 text-slate-300' 
                    : 'bg-gray-200 hover:bg-gray-300 text-gray-600'
                }`}
              >
                暂停
              </button>
            ) : (
              <button
                onClick={startAutoRun}
                disabled={running}
                className={`px-3 py-1.5 text-xs font-medium rounded transition-colors ${
                  isDark 
                    ? 'bg-blue-600 hover:bg-blue-500 text-white disabled:bg-blue-800' 
                    : 'bg-blue-600 hover:bg-blue-500 text-white disabled:bg-blue-300'
                } disabled:cursor-not-allowed`}
              >
                自动分析
              </button>
            )}
          </div>
        )}
        
        {session && !session.is_complete && (
          <div className={`text-[10px] ${isDark ? 'text-slate-500' : 'text-gray-400'}`}>
            候选关系: {session.candidates_count}
          </div>
        )}
      </div>
    </div>
  );
}

// 候选关系行组件
function CandidateRow({ candidate }: { candidate: HarnessRelationCandidate }) {
  const { theme } = useThemeStore();
  const isDark = theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  
  return (
    <div className={`flex items-center gap-1.5 p-1.5 rounded text-[10px] ${
      isDark ? 'bg-[#0f172a]/50' : 'bg-white'
    }`}>
      {candidate.verified ? (
        <CheckCircle size={12} className="text-green-400 flex-shrink-0" />
      ) : (
        <Clock size={12} className="text-yellow-400 flex-shrink-0" />
      )}
      
      <span className={`font-mono ${isDark ? 'text-blue-400' : 'text-blue-600'}`}>
        {candidate.source_table}
      </span>
      <span className="text-slate-500">.</span>
      <span className={`font-mono ${isDark ? 'text-slate-300' : 'text-gray-600'}`}>
        {candidate.source_column}
      </span>
      
      <ArrowRight size={10} className={`flex-shrink-0 mx-0.5 ${isDark ? 'text-slate-500' : 'text-gray-400'}`} />
      
      <span className={`font-mono ${isDark ? 'text-blue-400' : 'text-blue-600'}`}>
        {candidate.target_table}
      </span>
      <span className="text-slate-500">.</span>
      <span className={`font-mono ${isDark ? 'text-slate-300' : 'text-gray-600'}`}>
        {candidate.target_column}
      </span>
      
      <span className={`flex-shrink-0 ml-auto px-1 py-0.5 rounded text-[9px] ${
        candidate.confidence >= 0.8 
          ? 'bg-green-500/20 text-green-400' 
          : candidate.confidence >= 0.5 
            ? 'bg-yellow-500/20 text-yellow-400'
            : 'bg-gray-500/20 text-gray-400'
      }`}>
        {Math.round(candidate.confidence * 100)}%
      </span>
    </div>
  );
}
