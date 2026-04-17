// import { useEffect, useRef, useState, useCallback } from 'react';
// import cytoscape from 'cytoscape';
// import { 
//   Loader2, ZoomIn, ZoomOut, Maximize, Download, RefreshCw, 
//   Save, FolderOpen, Trash2, FileCode, Image as ImageIcon, Wand2 
// } from 'lucide-react';
// import { databaseApi, llmApi } from '../../utils/api';
// import type { DatabaseMetadata, TableMetadata, ColumnInfo, TableRelationAnalysis } from '../../types';
// import ConfirmModal from '../common/ConfirmModal';

// interface ChartDBVisualizerProps {
//   connectionId: string;
//   database: string;
//   schema?: string;
// }

// interface TablePosition {
//   x: number;
//   y: number;
// }

// export default function ChartDBVisualizer({ connectionId, database, schema }: ChartDBVisualizerProps) {
//   const containerRef = useRef<HTMLDivElement>(null);
//   const cyRef = useRef<cytoscape.Core | null>(null);
//   const metadataRef = useRef<DatabaseMetadata | null>(null);
//   const positionsRef = useRef<Map<string, TablePosition>>(new Map());
  
//   const [loading, setLoading] = useState(false);
//   const [metadata, setMetadata] = useState<DatabaseMetadata | null>(null);
//   const [error, setError] = useState<string | null>(null);
//   const [selectedNode, setSelectedNode] = useState<any>(null);
//   const [confirmClear, setConfirmClear] = useState(false);
//   const [showExportMenu, setShowExportMenu] = useState(false);
//   const [, forceUpdate] = useState({});
  
//   // LLM 分析相关状态
//   const [llmRelations, setLlmRelations] = useState<TableRelationAnalysis[]>([]);
//   const [llmLoading, setLlmLoading] = useState(false);
//   const [llmError, setLlmError] = useState<string | null>(null);
//   const [hasLlmCache, setHasLlmCache] = useState(false);

//   // 计算表节点位置（网格布局，但有偏移避免重叠）
//   const calculateTablePositions = useCallback((tables: TableMetadata[]): Map<string, TablePosition> => {
//     const positions = new Map<string, TablePosition>();
//     const cardWidth = 220;
//     const cardHeight = 300;
//     const spacingX = 100;
//     const spacingY = 80;
//     const cols = Math.ceil(Math.sqrt(tables.length));
    
//     tables.forEach((table, index) => {
//       const row = Math.floor(index / cols);
//       const col = index % cols;
      
//       // 添加随机偏移避免完全对齐
//       const randomOffsetX = (Math.random() - 0.5) * 40;
//       const randomOffsetY = (Math.random() - 0.5) * 40;
      
//       positions.set(table.name, {
//         x: col * (cardWidth + spacingX) + 150 + randomOffsetX,
//         y: row * (cardHeight + spacingY) + 100 + randomOffsetY
//       });
//     });
    
//     return positions;
//   }, []);

//   // 创建表节点（卡片式）
//   const createTableNodes = useCallback((metadata: DatabaseMetadata, positions: Map<string, TablePosition>) => {
//     const nodes: cytoscape.ElementDefinition[] = [];
    
//     metadata.tables.forEach(table => {
//       const pos = positions.get(table.name) || { x: 100, y: 100 };
      
//       nodes.push({
//         data: {
//           id: `table-${table.name}`,
//           label: table.name,
//           type: 'table',
//           tableData: table,
//           width: 220,
//           height: Math.max(80, 60 + table.columns.length * 28)
//         },
//         position: { ...pos }
//       });
//     });
    
//     return nodes;
//   }, []);

//   // 创建外键关系边
//   const createForeignKeyEdges = useCallback((metadata: DatabaseMetadata): cytoscape.ElementDefinition[] => {
//     const edges: cytoscape.ElementDefinition[] = [];
    
//     metadata.tables.forEach(table => {
//       table.foreign_keys.forEach((fk, index) => {
//         edges.push({
//           data: {
//             id: `fk-${table.name}-${fk.column_name}-${index}`,
//             source: `table-${table.name}`,
//             target: `table-${fk.referenced_table}`,
//             label: `${fk.column_name} → ${fk.referenced_column}`,
//             type: 'foreign_key',
//             fkData: fk
//           }
//         });
//       });
//     });

//     return edges;
//   }, []);

//   // 创建 LLM 关系边
//   const createLlmRelationEdges = useCallback((metadata: DatabaseMetadata): cytoscape.ElementDefinition[] => {
//     const edges: cytoscape.ElementDefinition[] = [];
    
//     if (!metadata.llm_relations) return edges;
    
//     metadata.llm_relations.forEach((rel, index) => {
//       edges.push({
//         data: {
//           id: `llm-rel-${index}`,
//           source: `table-${rel.source_table}`,
//           target: `table-${rel.target_table}`,
//           label: `${rel.relation_type} (${Math.round(rel.confidence * 100)}%)`,
//           type: 'llm_relation',
//           llmData: rel
//         }
//       });
//     });

//     return edges;
//   }, []);

//   const initCy = useCallback((data: DatabaseMetadata) => {
//     if (!containerRef.current) return;

//     if (cyRef.current) {
//       cyRef.current.destroy();
//     }

//     metadataRef.current = data;
    
//     // 计算表位置
//     const positions = calculateTablePositions(data.tables);
//     positionsRef.current = positions;
    
//     const tableNodes = createTableNodes(data, positions);
//     const fkEdges = createForeignKeyEdges(data);
//     const llmEdges = createLlmRelationEdges(data);

//     const cy = cytoscape({
//       container: containerRef.current,
//       elements: [
//         ...tableNodes,
//         ...fkEdges,
//         ...llmEdges
//       ],
//       style: [
//         // ===== 表节点 - 卡片式 =====
//         {
//           selector: 'node[type="table"]',
//           style: {
//             'shape': 'rectangle',
//             'width': 'data(width)',
//             'height': 'data(height)',
//             'background-color': '#1e293b',
//             'border-width': 1,
//             'border-color': '#334155',
//             'border-radius': 8,
//             'label': '',
//             'shadow-blur': 15,
//             'shadow-color': 'rgba(0,0,0,0.5)',
//             'shadow-offset-y': 5
//           }
//         },
//         // ===== 外键关系 - 实线 =====
//         {
//           selector: 'edge[type="foreign_key"]',
//           style: {
//             'width': 2,
//             'line-color': '#64748b',
//             'target-arrow-color': '#64748b',
//             'target-arrow-shape': 'triangle',
//             'curve-style': 'bezier',
//             'arrow-scale': 1.2,
//             'label': 'data(label)',
//             'font-size': '9px',
//             'color': '#94a3b8',
//             'text-background-color': '#0f172a',
//             'text-background-opacity': 0.9,
//             'text-background-padding': '3px'
//           }
//         },
//         // ===== LLM 关系 - 虚线 =====
//         {
//           selector: 'edge[type="llm_relation"]',
//           style: {
//             'width': 2,
//             'line-color': '#a855f7',
//             'target-arrow-color': '#a855f7',
//             'target-arrow-shape': 'triangle',
//             'curve-style': 'bezier',
//             'arrow-scale': 1.2,
//             'line-style': 'dashed',
//             'line-dash-pattern': [6, 4],
//             'label': 'data(label)',
//             'font-size': '9px',
//             'color': '#a855f7',
//             'text-background-color': '#0f172a',
//             'text-background-opacity': 0.9,
//             'text-background-padding': '3px'
//           }
//         },
//         // ===== 选中状态 =====
//         {
//           selector': ':selected',
//           style: {
//             'border-width': 3,
//             'border-color': '#3b82f6',
//             'shadow-blur': 25,
//             'shadow-color': 'rgba(59,130,246,0.5)'
//           }
//         },
//         // ===== 悬停状态 =====
//         {
//           selector: 'node[type="table"]:hover',
//           style: {
//             'border-color': '#60a5fa',
//             'shadow-blur': 25,
//             'shadow-color': 'rgba(96,165,250,0.4)'
//           }
//         }
//       ],
//       layout: {
//         name: 'preset'
//       },
//       minZoom: 0.2,
//       maxZoom: 2,
//       wheelSensitivity: 0.3
//     });

//     // 点击表节点
//     cy.on('tap', 'node[type="table"]', (evt) => {
//       setSelectedNode(evt.target.data());
//     });

//     cy.on('tap', (evt) => {
//       if (evt.target === cy) {
//         setSelectedNode(null);
//       }
//     });

//     // 拖拽结束保存位置
//     cy.on('dragfree', 'node[type="table"]', (evt) => {
//       const node = evt.target;
//       const tableName = node.data('label');
//       positionsRef.current.set(tableName, { ...node.position() });
//     });

//     cyRef.current = cy;
//   }, [calculateTablePositions, createTableNodes, createForeignKeyEdges, createLlmRelationEdges]);

//   const loadMetadata = useCallback(async () => {
//     setLoading(true);
//     setError(null);
//     try {
//       const data = await databaseApi.getDatabaseMetadata(connectionId, database, schema);
//       setMetadata(data);
//       initCy(data);
//     } catch (e) {
//       setError(String(e));
//     } finally {
//       setLoading(false);
//     }
//   }, [connectionId, database, schema, initCy]);

//   // 加载 LLM 分析结果
//   const loadLlmRelations = useCallback(async () => {
//     setLlmLoading(true);
//     setLlmError(null);
//     try {
//       const response = await llmApi.getTableRelations(connectionId, database, schema);
//       setLlmRelations(response.relations);
//       setHasLlmCache(response.from_cache);
//     } catch (e) {
//       setLlmError(String(e));
//     } finally {
//       setLlmLoading(false);
//     }
//   }, [connectionId, database, schema]);

//   // 刷新 LLM 分析
//   const refreshLlmRelations = useCallback(async () => {
//     setLlmLoading(true);
//     setLlmError(null);
//     try {
//       const response = await llmApi.refreshTableRelations(connectionId, database, schema);
//       setLlmRelations(response.relations);
//       setHasLlmCache(false);
//     } catch (e) {
//       setLlmError(String(e));
//     } finally {
//       setLlmLoading(false);
//     }
//   }, [connectionId, database, schema]);

//   // 导出 DDL
//   const exportDDL = useCallback(() => {
//     if (!metadata) return;
    
//     let ddl = `-- Database: ${database}\n-- Generated by BaizeDB\n\n`;
    
//     metadata.tables.forEach(table => {
//       ddl += `CREATE TABLE \`${table.name}\` (\n`;
      
//       table.columns.forEach((col, index) => {
//         const isLast = index === table.columns.length - 1 && table.foreign_keys.length === 0;
//         ddl += `  \`${col.name}\` ${col.data_type}${col.nullable ? '' : ' NOT NULL'}${isLast ? '' : ','}\n`;
//       });
      
//       table.foreign_keys.forEach((fk, index) => {
//         const isLast = index === table.foreign_keys.length - 1;
//         ddl += `  FOREIGN KEY (\`${fk.column_name}\`) REFERENCES \`${fk.referenced_table}\`(\`${fk.referenced_column}\`)${isLast ? '' : ','}\n`;
//       });
      
//       ddl += `);\n\n`;
//     });
    
//     const blob = new Blob([ddl], { type: 'text/sql' });
//     const url = URL.createObjectURL(blob);
//     const link = document.createElement('a');
//     link.href = url;
//     link.download = `${database}_schema.sql`;
//     link.click();
//     URL.revokeObjectURL(url);
//     setShowExportMenu(false);
//   }, [metadata, database]);

//   // 导出图片
//   const exportImage = useCallback(() => {
//     if (!cyRef.current) return;
//     const png = cyRef.current.png({
//       bg: '#0f172a',
//       full: true,
//       scale: 2
//     });
//     const link = document.createElement('a');
//     link.href = png;
//     link.download = `schema-${database}-${Date.now()}.png`;
//     link.click();
//     setShowExportMenu(false);
//   }, [database]);

//   const zoomIn = useCallback(() => {
//     cyRef.current?.zoom(cyRef.current.zoom() * 1.2);
//   }, []);

//   const zoomOut = useCallback(() => {
//     cyRef.current?.zoom(cyRef.current.zoom() / 1.2);
//   }, []);

//   const fit = useCallback(() => {
//     cyRef.current?.fit();
//   }, []);

//   useEffect(() => {
//     loadMetadata();
//     loadLlmRelations();
//   }, [loadMetadata, loadLlmRelations]);

//   // 渲染表卡片内容
//   const renderTableCard = (tableData: TableMetadata) => {
//     return (
//       <div className="w-[220px] bg-[#1e293b] rounded-lg border border-[#334155] overflow-hidden shadow-lg">
//         {/* 表头 */}
//         <div className="px-3 py-2 bg-[#334155] border-b border-[#475569]">
//           <span className="text-sm font-semibold text-white">{tableData.name}</span>
//           {tableData.comment && (
//             <span className="text-xs text-gray-400 ml-2">{tableData.comment}</span>
//           )}
//         </div>
        
//         {/* 字段列表 */}
//         <div className="py-1">
//           {tableData.columns.map((col) => (
//             <div 
//               key={col.name}
//               className={`flex items-center justify-between px-3 py-1.5 text-xs ${
//                 col.key === 'PRI' ? 'bg-yellow-500/10' : 'hover:bg-[#334155]/50'
//               }`}
//             >
//               <div className="flex items-center gap-2">
//                 {col.key === 'PRI' && (
//                   <span className="w-2 h-2 rounded-full bg-yellow-500" title="Primary Key" />
//                 )}
//                 {tableData.foreign_keys.some(fk => fk.column_name === col.name) && (
//                   <span className="w-2 h-2 rounded-full bg-blue-500" title="Foreign Key" />
//                 )}
//                 <span className={`font-mono ${col.key === 'PRI' ? 'text-yellow-400' : 'text-gray-300'}`}>
//                   {col.name}
//                 </span>
//               </div>
//               <span className="text-gray-500">{col.data_type}</span>
//             </div>
//           ))}
//         </div>
//       </div>
//     );
//   };

//   return (
//     <div className="h-full flex flex-col">
//       {/* 工具栏 */}
//       <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0">
//         <span className="text-sm font-medium text-[var(--text-primary)]">
//           {database}{schema ? `.${schema}` : ''}
//         </span>
//         <div className="flex-1" />
        
//         <button 
//           className="btn-ghost py-1 px-2 text-xs text-purple-400" 
//           onClick={refreshLlmRelations} 
//           disabled={llmLoading} 
//           title="AI 分析表关系"
//         >
//           {llmLoading ? <Loader2 size={14} className="animate-spin" /> : <Wand2 size={14} />}
//           <span className="ml-1">AI 分析{hasLlmCache && ' ✓'}</span>
//         </button>
        
//         <button className="btn-ghost py-1 px-2 text-xs" onClick={loadMetadata} disabled={loading} title="刷新">
//           {loading ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
//           <span className="ml-1">刷新</span>
//         </button>
        
//         <div className="relative">
//           <button 
//             className="btn-ghost py-1 px-2 text-xs text-green-400" 
//             onClick={() => setShowExportMenu(!showExportMenu)}
//             title="导出"
//           >
//             <Download size={14} />
//             <span className="ml-1">导出</span>
//           </button>
          
//           {showExportMenu && (
//             <div className="absolute right-0 top-full mt-1 py-1 bg-[var(--bg-primary)] border border-[var(--border)] rounded-lg shadow-lg z-50 min-w-[120px]">
//               <button 
//                 className="w-full px-3 py-2 text-xs text-left hover:bg-[var(--bg-secondary)] flex items-center gap-2"
//                 onClick={exportDDL}
//               >
//                 <FileCode size={14} />
//                 导出 DDL
//               </button>
//               <button 
//                 className="w-full px-3 py-2 text-xs text-left hover:bg-[var(--bg-secondary)] flex items-center gap-2"
//                 onClick={exportImage}
//               >
//                 <ImageIcon size={14} />
//                 导出图片
//               </button>
//             </div>
//           )}
//         </div>
        
//         <div className="h-4 w-px bg-[var(--border)]" />
        
//         <button className="btn-ghost py-1 px-2 text-xs" onClick={zoomOut} title="缩小">
//           <ZoomOut size={14} />
//         </button>
//         <button className="btn-ghost py-1 px-2 text-xs" onClick={fit} title="适应">
//           <Maximize size={14} />
//         </button>
//         <button className="btn-ghost py-1 px-2 text-xs" onClick={zoomIn} title="放大">
//           <ZoomIn size={14} />
//         </button>
//       </div>

//       {/* 主内容区 */}
//       <div className="flex-1 flex overflow-hidden">
//         {/* 图区域 */}
//         <div className="flex-1 relative">
//           <div ref={containerRef} className="w-full h-full bg-[#0f172a]" />
          
//           {/* 自定义表卡片渲染层 */}
//           {cyRef.current && metadata && (
//             <div className="absolute inset-0 pointer-events-none overflow-hidden">
//               {metadata.tables.map(table => {
//                 const pos = positionsRef.current.get(table.name);
//                 if (!pos) return null;
                
//                 return (
//                   <div
//                     key={table.name}
//                     className="absolute pointer-events-auto"
//                     style={{
//                       left: pos.x - 110,
//                       top: pos.y - 30,
//                       width: 220
//                     }}
//                   >
//                     {renderTableCard(table)}
//                   </div>
//                 );
//               })}
//             </div>
//           )}
          
//           {/* 图例 */}
//           <div className="absolute bottom-3 left-3 px-3 py-2.5 bg-[#0f172a]/90 border border-[#334155] rounded-lg text-[10px] text-slate-400 backdrop-blur-sm shadow-lg">
//             <div className="flex flex-col gap-1.5">
//               <div className="flex items-center gap-2">
//                 <span className="inline-block w-3 h-3 rounded-sm bg-[#1e293b] border border-[#334155]" />
//                 <span>数据表</span>
//               </div>
//               <div className="flex items-center gap-2">
//                 <span className="inline-block w-2 h-2 rounded-full bg-yellow-500" />
//                 <span>主键字段</span>
//               </div>
//               <div className="flex items-center gap-2">
//                 <span className="inline-block w-2 h-2 rounded-full bg-blue-500" />
//                 <span>外键字段</span>
//               </div>
//               <div className="h-px bg-[#334155] my-0.5" />
//               <div className="flex items-center gap-2">
//                 <span className="inline-block w-5 h-0 border-t-2 border-[#64748b]" />
//                 <span>外键关系</span>
//               </div>
//               <div className="flex items-center gap-2">
//                 <span className="inline-block w-5 h-0 border-t-2 border-dashed border-[#a855f7]" />
//                 <span>AI 推测关系</span>
//               </div>
//             </div>
//             {llmRelations.length > 0 && (
//               <div className="mt-2 pt-1.5 border-t border-[#334155] text-purple-400">
//                 AI 发现 {llmRelations.length} 个关系
//                 {hasLlmCache && <span className="ml-1 text-green-400">✓ 已缓存</span>}
//               </div>
//             )}
//           </div>
          
//           {loading && (
//             <div className="absolute inset-0 flex items-center justify-center bg-black/20">
//               <div className="flex items-center gap-2 px-4 py-2 bg-[var(--bg-primary)] rounded-lg shadow-lg">
//                 <Loader2 size={16} className="animate-spin" />
//                 <span className="text-sm">加载中...</span>
//               </div>
//             </div>
//           )}
          
//           {error && (
//             <div className="absolute inset-0 flex items-center justify-center">
//               <div className="px-4 py-3 bg-red-500/10 border border-red-500/30 rounded-lg text-red-400 max-w-md">
//                 <p className="text-sm font-medium mb-1">加载失败</p>
//                 <p className="text-xs">{error}</p>
//                 <button className="mt-2 text-xs text-red-400 hover:text-red-300 underline" onClick={loadMetadata}>
//                   重试
//                 </button>
//               </div>
//             </div>
//           )}
          
//           {llmError && (
//             <div className="absolute top-20 left-3 right-3 max-w-md mx-auto">
//               <div className="px-3 py-2 bg-purple-500/10 border border-purple-500/30 rounded-lg text-purple-400 text-xs">
//                 <p className="font-medium mb-1">AI 分析失败</p>
//                 <p>{llmError}</p>
//               </div>
//             </div>
//           )}
//         </div>

//         {/* 详情面板 */}
//         {selectedNode && (
//           <div className="w-72 border-l border-[var(--border)] bg-[var(--bg-secondary)] p-4 overflow-y-auto flex-shrink-0">
//             <div>
//               <div className="flex items-center gap-2 mb-3">
//                 <span className="px-2 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded">TABLE</span>
//                 <h3 className="text-sm font-semibold text-[var(--text-primary)]">{selectedNode.label}</h3>
//               </div>
              
//               {selectedNode.tableData?.comment && (
//                 <p className="text-xs text-[var(--text-muted)] mb-3 p-2 bg-[var(--bg-primary)] rounded">{selectedNode.tableData.comment}</p>
//               )}
              
//               <div className="space-y-2 text-xs">
//                 <div className="flex justify-between py-1 border-b border-[var(--border)]">
//                   <span className="text-[var(--text-muted)]">字段数</span>
//                   <span className="font-mono">{selectedNode.tableData?.columns.length || 0}</span>
//                 </div>
//                 <div className="flex justify-between py-1 border-b border-[var(--border)]">
//                   <span className="text-[var(--text-muted)]">外键数</span>
//                   <span className="font-mono">{selectedNode.tableData?.foreign_keys.length || 0}</span>
//                 </div>
//                 <div className="flex justify-between py-1 border-b border-[var(--border)]">
//                   <span className="text-[var(--text-muted)]">被引用数</span>
//                   <span className="font-mono">{selectedNode.tableData?.referenced_by.length || 0}</span>
//                 </div>
//               </div>

//               {/* 表结构列表 */}
//               <div className="mt-4">
//                 <h4 className="text-xs font-medium text-[var(--text-muted)] mb-2">表结构</h4>
//                 <div className="space-y-1 max-h-72 overflow-y-auto">
//                   {selectedNode.tableData?.columns.map((col: ColumnInfo) => (
//                     <div 
//                       key={col.name}
//                       className={`flex items-center gap-2 px-2 py-1.5 rounded text-xs ${
//                         col.key === 'PRI' ? 'bg-yellow-500/10 border border-yellow-500/30' : 'bg-[var(--bg-primary)]'
//                       }`}
//                     >
//                       <span className="text-[var(--text-muted)] w-6 text-center font-mono text-[10px]">
//                         {col.key === 'PRI' ? 'PK' : col.nullable ? '' : '!'}
//                       </span>
//                       <span className="flex-1 truncate font-mono">{col.name}</span>
//                       <span className="text-[var(--text-muted)] text-[10px]">{col.data_type}</span>
//                     </div>
//                   ))}
//                 </div>
//               </div>
//             </div>
//           </div>
//         )}
//       </div>

//       {/* 确认弹窗 */}
//       {confirmClear && (
//         <ConfirmModal
//           message="确定清除本地保存的可视化数据吗？此操作不可撤销。"
//           onConfirm={() => {}}
//           onCancel={() => setConfirmClear(false)}
//           danger
//         />
//       )}
//     </div>
//   );
// }
