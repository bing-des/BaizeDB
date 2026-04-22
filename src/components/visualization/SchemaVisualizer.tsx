import { useEffect, useRef, useState, useCallback } from 'react';
import cytoscape from 'cytoscape';
import fcose from 'cytoscape-fcose';
import { Loader2, ZoomIn, ZoomOut, Maximize, Download, RefreshCw, Save, FolderOpen, Trash2 } from 'lucide-react';

// 注册 fcose 布局
cytoscape.use(fcose);
import { databaseApi } from '../../utils/api';
import { useThemeStore } from '../../store';
import type { DatabaseMetadata, TableMetadata, ColumnInfo } from '../../types';
import ConfirmModal from '../common/ConfirmModal';

interface SchemaVisualizerProps {
  connectionId: string;
  database: string;
  schema?: string;
}

interface ExpandedTable {
  tableName: string;
  columnNodeIds: string[];
  edgeIds: string[];
}

export default function SchemaVisualizer({ connectionId, database, schema }: SchemaVisualizerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<cytoscape.Core | null>(null);
  const metadataRef = useRef<DatabaseMetadata | null>(null);
  const expandedTablesRef = useRef<Map<string, ExpandedTable>>(new Map());
  const { theme } = useThemeStore();
  
  const [loading, setLoading] = useState(false);
  const [metadata, setMetadata] = useState<DatabaseMetadata | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<any>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [, forceUpdate] = useState({});

  // 判断当前是否为深色主题
  const isDark = theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  // 计算字段节点位置（星形放射状布局，确保不重叠）
  const calculateColumnPositions = useCallback((tableX: number, tableY: number, columnCount: number) => {
    const positions: { x: number; y: number }[] = [];
    
    // 根据字段数量动态计算半径，确保不重叠
    const nodeSize = 50; // 字段节点大小
    const minGap = 20; // 最小间距
    const minRadius = 130; // 最小半径
    
    // 计算所需周长：字段数量 * (节点大小 + 间距)
    const requiredCircumference = columnCount * (nodeSize + minGap);
    // 计算半径：周长 / 2π
    const calculatedRadius = requiredCircumference / (2 * Math.PI);
    // 取较大值确保足够空间
    const radius = Math.max(minRadius, calculatedRadius);
    
    // 均匀分布在圆周上（从顶部开始）
    const angleStep = (2 * Math.PI) / Math.max(columnCount, 1);
    
    for (let i = 0; i < columnCount; i++) {
      // 从顶部开始（-PI/2），顺时针均匀分布
      const angle = i * angleStep - Math.PI / 2;
      
      positions.push({
        x: tableX + Math.cos(angle) * radius,
        y: tableY + Math.sin(angle) * radius
      });
    }
    
    return positions;
  }, []);

  const createTableNode = useCallback((table: TableMetadata, position: { x: number; y: number }) => {
    return {
      data: {
        id: `table-${table.name}`,
        label: table.name,
        type: 'table',
        tableData: table,
        expanded: false
      },
      position: { ...position }
    };
  }, []);

  const createColumnNodesAndEdges = useCallback((table: TableMetadata, tablePosition: { x: number; y: number }) => {
    const nodes: cytoscape.ElementDefinition[] = [];
    const edges: cytoscape.ElementDefinition[] = [];
    const tableId = `table-${table.name}`;
    
    const positions = calculateColumnPositions(tablePosition.x, tablePosition.y, table.columns.length);

    table.columns.forEach((col, index) => {
      const colId = `col-${table.name}-${col.name}`;
      const pos = positions[index];
      
      nodes.push({
        data: {
          id: colId,
          label: col.name,
          type: 'column',
          parentTable: table.name,
          columnData: {
            name: col.name,
            data_type: col.data_type,
            nullable: col.nullable,
            key: col.key || undefined,
            comment: col.comment
          }
        },
        position: pos
      });
      
      edges.push({
        data: {
          id: `edge-${tableId}-${colId}`,
          source: tableId,
          target: colId,
          type: 'table_column'
        }
      });
    });

    return { nodes, edges };
  }, [calculateColumnPositions]);

  const createForeignKeyEdges = useCallback((metadata: DatabaseMetadata, expandedTables: Set<string>): cytoscape.ElementDefinition[] => {
    const edges: cytoscape.ElementDefinition[] = [];
    
    metadata.tables.forEach(table => {
      table.foreign_keys.forEach((fk, fkIndex) => {
        const sourceTable = table.name;
        const targetTable = fk.referenced_table;
        const sourceExpanded = expandedTables.has(sourceTable);
        const targetExpanded = expandedTables.has(targetTable);
        
        // 两个表都展开：列到列
        if (sourceExpanded && targetExpanded) {
          const sourceColId = `col-${sourceTable}-${fk.column_name}`;
          const targetColId = `col-${targetTable}-${fk.referenced_column}`;
          
          edges.push({
            data: {
              id: `fk-${sourceTable}-${fk.column_name}-${fkIndex}`,
              source: sourceColId,
              target: targetColId,
              label: '',
              type: 'foreign_key',
              fkData: fk
            }
          });
        }
        // 源表展开，目标表未展开：源字段 → 目标表
        else if (sourceExpanded && !targetExpanded) {
          const sourceColId = `col-${sourceTable}-${fk.column_name}`;
          const targetTableId = `table-${targetTable}`;
          
          edges.push({
            data: {
              id: `fk-col-table-${sourceTable}-${fk.column_name}-${fkIndex}`,
              source: sourceColId,
              target: targetTableId,
              label: `${fk.column_name} → ${fk.referenced_column}`,
              type: 'foreign_key_table',
              fkData: fk
            }
          });
        }
        // 源表未展开，目标表展开：源表 → 目标字段
        else if (!sourceExpanded && targetExpanded) {
          const sourceTableId = `table-${sourceTable}`;
          const targetColId = `col-${targetTable}-${fk.referenced_column}`;
          
          edges.push({
            data: {
              id: `fk-table-col-${sourceTable}-${fk.column_name}-${fkIndex}`,
              source: sourceTableId,
              target: targetColId,
              label: `${fk.column_name} → ${fk.referenced_column}`,
              type: 'foreign_key_table',
              fkData: fk
            }
          });
        }
        // 两个表都未展开：表到表
        else {
          const sourceTableId = `table-${sourceTable}`;
          const targetTableId = `table-${targetTable}`;
          
          edges.push({
            data: {
              id: `fk-table-${sourceTable}-${fk.column_name}-${fkIndex}`,
              source: sourceTableId,
              target: targetTableId,
              label: `${fk.column_name} → ${fk.referenced_column}`,
              type: 'foreign_key_table',
              fkData: fk
            }
          });
        }
      });
    });

    return edges;
  }, []);



  const initCy = useCallback((data: DatabaseMetadata, darkMode: boolean) => {
    if (!containerRef.current) return;

    if (cyRef.current) {
      cyRef.current.destroy();
      expandedTablesRef.current.clear();
    }

    metadataRef.current = data;
    
    // 根据主题设置颜色
    const colors = darkMode ? {
      tableBg: '#3b82f6',
      tableBorder: '#60a5fa',
      tableExpandedBg: '#2563eb',
      tableExpandedBorder: '#1d4ed8',
      columnBg: '#60a5fa',
      columnBorder: '#93c5fd',
      columnText: '#9ca3af',
      tableColumnLine: '#9ca3af',
      fkLine: '#6b7280',
      fkText: '#6b7280',
      fkTextBg: '#ffffff',
    } : {
      tableBg: '#3b82f6',
      tableBorder: '#2563eb',
      tableExpandedBg: '#1d4ed8',
      tableExpandedBorder: '#1e40af',
      columnBg: '#60a5fa',
      columnBorder: '#3b82f6',
      columnText: '#4b5563',
      tableColumnLine: '#9ca3af',
      fkLine: '#6b7280',
      fkText: '#4b5563',
      fkTextBg: '#ffffff',
    };
    
    // 创建表节点（不预设位置，由布局算法决定）
    const initialNodes = data.tables.map((table) => {
      return createTableNode(table, { x: 0, y: 0 });
    });

    const fkEdges = createForeignKeyEdges(data, new Set());

    const cy = cytoscape({
      container: containerRef.current,
      elements: [
        ...initialNodes,
        // 初始化时添加表到表的关系连线
        ...fkEdges,
      ],
      style: [
        // ===== 表节点 - 3D立体效果 =====
        {
          selector: 'node[type="table"]',
          style: ({
            'background-color': colors.tableBg,
            'label': 'data(label)',
            'width': 90,
            'height': 90,
            'shape': 'ellipse',
            'text-valign': 'bottom',
            'text-halign': 'center',
            'color': colors.tableBg,
            'font-size': '12px',
            'font-weight': 'bold',
            'text-margin-y': 10,
            'border-width': 0,
            // 3D阴影效果
            'shadow-blur': 20,
            'shadow-color': isDark ? 'rgba(0,0,0,0.6)' : 'rgba(0,0,0,0.3)',
            'shadow-offset-x': 0,
            'shadow-offset-y': 8,
            // 渐变效果（通过背景色模拟）
            'background-opacity': 0.95
          } as any)
        },
        {
          selector: 'node[type="table"][expanded = "true"]',
          style: ({
            'background-color': colors.tableExpandedBg,
            'shadow-blur': 30,
            'shadow-color': isDark ? 'rgba(59,130,246,0.5)' : 'rgba(59,130,246,0.3)',
            'shadow-offset-y': 12
          } as any)
        },
        // ===== 列节点 - 3D立体效果 =====
        {
          selector: 'node[type="column"]',
          style: ({
            'background-color': colors.columnBg,
            'label': 'data(label)',
            'width': 50,
            'height': 50,
            'shape': 'ellipse',
            'text-valign': 'center',
            'text-halign': 'center',
            'color': '#ffffff',
            'font-size': '8px',
            'font-weight': 500,
            'text-wrap': 'wrap',
            'text-max-width': '42px',
            'border-width': 0,
            // 3D阴影效果
            'shadow-blur': 12,
            'shadow-color': isDark ? 'rgba(0,0,0,0.5)' : 'rgba(0,0,0,0.25)',
            'shadow-offset-x': 0,
            'shadow-offset-y': 4,
            'background-opacity': 0.9
          } as any)
        },
        {
          selector: 'node[type="column"][columnData.key = "PRI"]',
          style: ({
            'background-color': '#f59e0b',
            'border-width': 3,
            'border-color': '#fbbf24',
            'shadow-color': 'rgba(245,158,11,0.6)',
            'shadow-blur': 15,
            'shadow-offset-y': 6
          } as any)
        },
        // ===== 节点悬停3D效果 =====
        {
          selector: 'node:hover',
          style: ({
            'shadow-blur': 25,
            'shadow-offset-y': 10,
            'shadow-color': isDark ? 'rgba(255,255,255,0.2)' : 'rgba(0,0,0,0.4)'
          } as any)
        },
        // ===== 表-列连接线 - 细直线 =====
        {
          selector: 'edge[type="table_column"]',
          style: {
            'width': 1,
            'line-color': colors.tableColumnLine,
            'curve-style': 'straight',
            'target-arrow-shape': 'none'
          }
        },
        // ===== 外键关系 - 带动画效果 =====
        {
          selector: 'edge[type="foreign_key"]',
          style: {
            'width': 2,
            'line-color': colors.fkLine,
            'target-arrow-color': colors.fkLine,
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'arrow-scale': 1.2,
            // 流动动画效果
            'line-dash-pattern': [8, 4],
            'line-dash-offset': 0
          }
        },
        {
          selector: 'edge[type="foreign_key_table"]',
          style: {
            'width': 2,
            'line-color': colors.fkLine,
            'target-arrow-color': colors.fkLine,
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'arrow-scale': 1.2,
            'label': 'data(label)',
            'font-size': '9px',
            'color': colors.fkText,
            'text-background-color': colors.fkTextBg,
            'text-background-opacity': 0.8,
            'text-background-padding': '2px',
            'line-dash-pattern': [8, 4],
            'line-dash-offset': 0
          }
        },

        // ===== 关系线悬停高亮 =====
        {
          selector: 'edge:hover',
          style: ({
            'width': 4,
            'shadow-blur': 10,
            'shadow-color': 'rgba(255,255,255,0.5)'
          } as any)
        },
        // ===== 选中状态 =====
        {
          selector: ':selected',
          style: {
            'border-width': 4,
            'border-color': '#f59e0b'
          }
        }
      ],
      layout: {
        name: 'preset'
      },
      minZoom: 0.1,
      maxZoom: 3,
      wheelSensitivity: 0.3
    });

    // 使用 fcose 布局自动排列表节点
    const layout = cy.layout({
      name: 'fcose',
      // 基本设置
      fit: true,
      padding: 80,
      animate: true,
      animationDuration: 600,
      // 节点间距 - 增大让无关联节点更分散
      nodeSeparation: 300,
      // 理想边长
      idealEdgeLength: (edge: cytoscape.EdgeSingular) => {
        const type = edge.data('type');
        // 有关系的表之间距离较近（聚类）
        if (type?.includes('foreign_key')) {
          return 180;
        }
        // 表-列关系保持中等距离
        if (type === 'table_column') {
          return 120;
        }
        // 其他边（无关联的表之间）距离很远
        return 500;
      },
      // 节点排斥力 - 大幅增大让无关联节点分散
      nodeRepulsion: (node: cytoscape.NodeSingular) => {
        // 表节点排斥力非常大，避免聚集
        if (node.data('type') === 'table') {
          // 根据连接数调整：连接少的表排斥力更大（更分散）
          const connectedEdges = node.connectedEdges().length;
          return connectedEdges > 0 ? 15000 : 30000;
        }
        return 5000;
      },
      // 边弹性
      edgeElasticity: (edge: cytoscape.EdgeSingular) => {
        const type = edge.data('type');
        if (type?.includes('foreign_key')) {
          return 0.9; // 关系边弹性高，保持紧凑
        }
        if (type === 'table_column') {
          return 0.7; // 表列边中等弹性
        }
        return 0.1; // 无关联边弹性很低，容易被拉开
      },
      // 重力 - 降低让整体更松散
      gravity: 0.15,
      // 重力范围 - 减小让重力影响范围更小
      gravityRange: 2.0,
      // 重力强度
      gravityCompound: 0.5,
      // 采样类型
      samplingType: true,
      // 采样大小
      sampleSize: 25,
      // 迭代次数
      numIter: 3000,
      // 冷却因子
      coolingFactor: 0.995,
      // 最小温度
      minTemp: 0.5,
      // 初始温度
      initialTemp: 2000,
      // 收敛阈值
      threshold: 0.0005,
      // 避免重叠
      avoidOverlap: true,
      avoidOverlapPadding: 30,
    } as any);

    layout.run();

    // 点击表节点 - 展开/收起切换，并显示右侧表结构
    cy.on('tap', 'node[type="table"]', (evt) => {
      const node = evt.target;
      const tableName = node.data('label');
      const isExpanded = expandedTablesRef.current.has(tableName);
      const tablePosition = node.position();
      const metadata = metadataRef.current;

      if (!metadata) return;

      // 移除所有现有的关系边（外键），展开/收起后会重新计算
      cy.edges('[type="foreign_key"], [type="foreign_key_table"]').remove();

      if (isExpanded) {
        // 收起
        const expanded = expandedTablesRef.current.get(tableName)!;
        [...expanded.columnNodeIds, ...expanded.edgeIds].forEach(id => {
          const el = cy.getElementById(id);
          if (el.length > 0) cy.remove(el);
        });
        expandedTablesRef.current.delete(tableName);
        node.data('expanded', 'false');
      } else {
        // 展开
        const table = metadata.tables.find(t => t.name === tableName);
        if (table) {
          const { nodes, edges } = createColumnNodesAndEdges(table, tablePosition);
          cy.add([...nodes, ...edges]);
          
          expandedTablesRef.current.set(tableName, {
            tableName,
            columnNodeIds: nodes.map(n => n.data.id as string),
            edgeIds: edges.map(e => e.data.id as string)
          });
          
          node.data('expanded', 'true');
        }
      }

      // 根据当前展开状态重新创建所有关系边
      const expandedTableNames = new Set(expandedTablesRef.current.keys());
      
      const newFkEdges = createForeignKeyEdges(metadata, expandedTableNames);
      newFkEdges.forEach(edge => {
        cy.add(edge);
      });

      // 设置选中的表节点，并标记为显示结构
      setSelectedNode({ ...node.data(), showStructure: true });
      forceUpdate({});
    });

    cy.on('tap', 'node[type="column"]', (evt) => {
      setSelectedNode(evt.target.data());
    });

    cy.on('tap', (evt) => {
      if (evt.target === cy) {
        setSelectedNode(null);
      }
    });

    cy.on('mouseover', 'node[type="table"]', (evt) => {
      evt.target.animate({
        style: { 'background-color': '#60a5fa' }
      }, { duration: 150 });
    });

    cy.on('mouseout', 'node[type="table"]', (evt) => {
      const isExpanded = expandedTablesRef.current.has(evt.target.data('label'));
      evt.target.animate({
        style: { 
          'background-color': isExpanded ? '#2563eb' : '#3b82f6'
        }
      }, { duration: 150 });
    });

    // 表拖动时，字段节点同步跟随
    let dragStartPos: { x: number; y: number } | null = null;
    let columnPositions: Map<string, { x: number; y: number }> = new Map();

    cy.on('grab', 'node[type="table"]', (evt) => {
      const tableNode = evt.target;
      const tableName = tableNode.data('label');
      const expanded = expandedTablesRef.current.get(tableName);
      
      if (expanded) {
        dragStartPos = { ...tableNode.position() };
        columnPositions.clear();
        
        // 记录所有字段节点相对于表节点的偏移
        expanded.columnNodeIds.forEach((colId: string) => {
          const colNode = cy.getElementById(colId);
          if (colNode.length > 0) {
            const colPos = colNode.position();
            columnPositions.set(colId, {
              x: colPos.x - dragStartPos!.x,
              y: colPos.y - dragStartPos!.y
            });
          }
        });
      }
    });

    cy.on('drag', 'node[type="table"]', (evt) => {
      const tableNode = evt.target;
      const tableName = tableNode.data('label');
      const expanded = expandedTablesRef.current.get(tableName);
      
      if (expanded && dragStartPos) {
        const currentPos = tableNode.position();
        
        // 同步移动所有字段节点
        expanded.columnNodeIds.forEach((colId: string) => {
          const offset = columnPositions.get(colId);
          if (offset) {
            const colNode = cy.getElementById(colId);
            if (colNode.length > 0) {
              colNode.position({
                x: currentPos.x + offset.x,
                y: currentPos.y + offset.y
              });
            }
          }
        });
      }
    });

    cy.on('free', 'node[type="table"]', () => {
      dragStartPos = null;
      columnPositions.clear();
    });

    // 点击表的聚效动画（脉冲效果）
    cy.on('tap', 'node[type="table"]', (evt) => {
      const node = evt.target;
      
      // 节点脉冲效果
      node.animate({
        style: {
          'shadow-blur': 50,
          'shadow-color': isDark ? 'rgba(59,130,246,0.8)' : 'rgba(59,130,246,0.6)'
        }
      }, { duration: 200 }).animate({
        style: {
          'shadow-blur': 20,
          'shadow-color': isDark ? 'rgba(0,0,0,0.6)' : 'rgba(0,0,0,0.3)'
        }
      }, { duration: 400 });
    });

    cyRef.current = cy;
  }, [createTableNode, createColumnNodesAndEdges, createForeignKeyEdges]);

  const loadMetadata = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await databaseApi.getDatabaseMetadata(connectionId, database, schema);
      setMetadata(data);
      initCy(data, isDark);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [connectionId, database, schema, initCy, isDark]);

  const saveToLocal = useCallback(async () => {
    if (!metadata) return;
    try {
      await databaseApi.saveVisualizationMetadata(connectionId, database, schema, metadata);
      alert('保存成功！');
    } catch (e) {
      alert(`保存失败: ${e}`);
    }
  }, [metadata, connectionId, database, schema]);

  const loadFromLocal = useCallback(async () => {
    try {
      const data = await databaseApi.loadVisualizationMetadata(connectionId, database, schema);
      if (data) {
        setMetadata(data);
        initCy(data, isDark);
      } else {
        alert('没有找到保存的数据');
      }
    } catch (e) {
      alert(`加载失败: ${e}`);
    }
  }, [connectionId, database, schema, initCy, isDark]);

  const clearLocalData = useCallback(async () => {
    try {
      await databaseApi.deleteVisualizationMetadata(connectionId, database, schema);
      setConfirmClear(false);
      alert('已清除本地数据');
    } catch (e) {
      alert(`清除失败: ${e}`);
    }
  }, [connectionId, database, schema]);

  const exportImage = useCallback(() => {
    if (!cyRef.current) return;
    const png = cyRef.current.png({
      bg: 'white',
      full: true,
      scale: 2
    });
    const link = document.createElement('a');
    link.href = png;
    link.download = `schema-${database}-${Date.now()}.png`;
    link.click();
  }, [database]);

  const zoomIn = useCallback(() => {
    cyRef.current?.zoom(cyRef.current.zoom() * 1.2);
  }, []);

  const zoomOut = useCallback(() => {
    cyRef.current?.zoom(cyRef.current.zoom() / 1.2);
  }, []);

  const fit = useCallback(() => {
    cyRef.current?.fit();
  }, []);

  const expandAll = useCallback(() => {
    if (!cyRef.current || !metadataRef.current) return;
    const cy = cyRef.current;
    const metadata = metadataRef.current;
    
    // 先移除所有现有关系边
    cy.edges('[type="foreign_key"], [type="foreign_key_table"]').remove();
    
    metadata.tables.forEach(table => {
      if (!expandedTablesRef.current.has(table.name)) {
        const tableNode = cy.getElementById(`table-${table.name}`);
        const tablePosition = tableNode.position();
        const { nodes, edges } = createColumnNodesAndEdges(table, tablePosition);
        cy.add([...nodes, ...edges]);
        
        expandedTablesRef.current.set(table.name, {
          tableName: table.name,
          columnNodeIds: nodes.map(n => n.data.id as string),
          edgeIds: edges.map(e => e.data.id as string)
        });
        tableNode.data('expanded', 'true');
      }
    });

    const expandedTableNames = new Set(expandedTablesRef.current.keys());
    
    // 重新创建外键关系边
    const newFkEdges = createForeignKeyEdges(metadata, expandedTableNames);
    newFkEdges.forEach(edge => {
      cy.add(edge);
    });
    
    forceUpdate({});
  }, [createColumnNodesAndEdges, createForeignKeyEdges]);

  const collapseAll = useCallback(() => {
    if (!cyRef.current || !metadataRef.current) return;
    const cy = cyRef.current;
    const metadata = metadataRef.current;
    
    // 先移除所有关系边
    cy.edges('[type="foreign_key"], [type="foreign_key_table"]').remove();
    
    // 移除所有展开的列节点
    expandedTablesRef.current.forEach((expanded) => {
      const tableNode = cy.getElementById(`table-${expanded.tableName}`);
      [...expanded.columnNodeIds, ...expanded.edgeIds].forEach(id => {
        const el = cy.getElementById(id);
        if (el.length > 0) cy.remove(el);
      });
      tableNode.data('expanded', 'false');
    });
    
    expandedTablesRef.current.clear();
    
    // 重新创建表到表的关系边
    const emptyExpanded = new Set<string>();
    const newFkEdges = createForeignKeyEdges(metadata, emptyExpanded);
    newFkEdges.forEach(edge => {
      cy.add(edge);
    });
    
    forceUpdate({});
  }, [createForeignKeyEdges]);

  useEffect(() => {
    loadMetadata();
  }, [loadMetadata]);

  // 主题变化时重新初始化图表
  useEffect(() => {
    if (metadataRef.current && cyRef.current) {
      initCy(metadataRef.current, isDark);
    }
  }, [isDark, initCy]);

  const getColumnTypeIcon = (dataType: string) => {
    const type = dataType.toLowerCase();
    if (type.includes('int')) return '123';
    if (type.includes('char') || type.includes('text')) return 'ABC';
    if (type.includes('date') || type.includes('time')) return '📅';
    if (type.includes('bool')) return '✓';
    if (type.includes('decimal') || type.includes('float') || type.includes('double')) return '#.#';
    return '?';
  };

  return (
    <div className="h-full flex flex-col">
      {/* 工具栏 */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border)] bg-[var(--bg-secondary)] flex-shrink-0">
        <span className="text-sm font-medium text-[var(--text-primary)]">
          {database}{schema ? `.${schema}` : ''}
        </span>
        <div className="flex-1" />
        
        <button className="btn-ghost py-1 px-2 text-xs" onClick={expandAll} title="展开全部">
          <span className="text-xs">↗ 展开全部</span>
        </button>
        
        <button className="btn-ghost py-1 px-2 text-xs" onClick={collapseAll} title="收起全部">
          <span className="text-xs">↙ 收起全部</span>
        </button>
        
        <div className="h-4 w-px bg-[var(--border)]" />
        
        <button className="btn-ghost py-1 px-2 text-xs" onClick={loadMetadata} disabled={loading} title="刷新">
          {loading ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
          <span className="ml-1">刷新</span>
        </button>
        
        <button className="btn-ghost py-1 px-2 text-xs text-green-400" onClick={saveToLocal} disabled={!metadata} title="保存">
          <Save size={14} />
          <span className="ml-1">保存</span>
        </button>
        
        <button className="btn-ghost py-1 px-2 text-xs" onClick={loadFromLocal} title="加载">
          <FolderOpen size={14} />
          <span className="ml-1">加载</span>
        </button>
        
        <div className="h-4 w-px bg-[var(--border)]" />
        
        <button className="btn-ghost py-1 px-2 text-xs" onClick={zoomOut} title="缩小">
          <ZoomOut size={14} />
        </button>
        <button className="btn-ghost py-1 px-2 text-xs" onClick={fit} title="适应">
          <Maximize size={14} />
        </button>
        <button className="btn-ghost py-1 px-2 text-xs" onClick={zoomIn} title="放大">
          <ZoomIn size={14} />
        </button>
        
        <div className="h-4 w-px bg-[var(--border)]" />
        
        <button className="btn-ghost py-1 px-2 text-xs text-blue-400" onClick={exportImage} disabled={!metadata} title="导出">
          <Download size={14} />
          <span className="ml-1">导出</span>
        </button>
        
        <button className="btn-ghost py-1 px-2 text-xs text-red-400" onClick={() => setConfirmClear(true)} title="清除">
          <Trash2 size={14} />
        </button>
      </div>

      {/* 主内容区 */}
      <div className="flex-1 flex overflow-hidden">
        {/* 图区域 */}
        <div className="flex-1 relative">
          <div ref={containerRef} className={`w-full h-full ${isDark ? 'bg-[#0f172a]' : 'bg-[#f8fafc]'}`} />
          
          {/* 图例 & 提示 */}
          <div className={`absolute bottom-3 left-3 px-3 py-2.5 border rounded-lg text-[10px] backdrop-blur-sm shadow-lg ${isDark ? 'bg-[#0f172a]/90 border-[#334155] text-slate-400' : 'bg-white/90 border-gray-200 text-gray-600'}`}>
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center gap-2">
                <span className="inline-block w-3 h-3 rounded-sm bg-[#3b82f6] border border-[#60a5fa]" />
                <span>表 (点击展开/收起)</span>
              </div>
              <div className="flex items-center gap-2">
                <span className={`inline-block w-2 h-2 rounded-full border ${isDark ? 'bg-[#60a5fa] border-[#93c5fd]' : 'bg-blue-400 border-blue-300'}`} />
                <span>普通字段</span>
              </div>
              <div className="flex items-center gap-2">
                <span className="inline-block w-2 h-2 rounded-full bg-[#fbbf24] border border-[#f59e0b]" />
                <span>主键字段</span>
              </div>
              <div className={`h-px my-0.5 ${isDark ? 'bg-[#334155]' : 'bg-gray-200'}`} />
              <div className="flex items-center gap-2">
                <span className={`inline-block w-5 h-0 border-t-2 ${isDark ? 'border-[#6b7280]' : 'border-gray-400'}`} />
                <span>外键关系</span>
              </div>
            </div>
          </div>
          
          {loading && (
            <div className="absolute inset-0 flex items-center justify-center bg-black/20">
              <div className="flex items-center gap-2 px-4 py-2 bg-[var(--bg-primary)] rounded-lg shadow-lg">
                <Loader2 size={16} className="animate-spin" />
                <span className="text-sm">加载中...</span>
              </div>
            </div>
          )}
          
          {error && (
            <div className="absolute inset-0 flex items-center justify-center">
              <div className="px-4 py-3 bg-red-500/10 border border-red-500/30 rounded-lg text-red-400 max-w-md">
                <p className="text-sm font-medium mb-1">加载失败</p>
                <p className="text-xs">{error}</p>
                <button className="mt-2 text-xs text-red-400 hover:text-red-300 underline" onClick={loadMetadata}>
                  重试
                </button>
              </div>
            </div>
          )}
          
        </div>

        {/* 详情面板 */}
        {selectedNode && (
          <div className="w-72 border-l border-[var(--border)] bg-[var(--bg-secondary)] p-4 overflow-y-auto flex-shrink-0">
            {selectedNode.type === 'table' ? (
              <div>
                <div className="flex items-center gap-2 mb-3">
                  <span className="px-2 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded">TABLE</span>
                  <h3 className="text-sm font-semibold text-[var(--text-primary)]">{selectedNode.label}</h3>
                </div>
                
                {selectedNode.tableData?.comment && (
                  <p className="text-xs text-[var(--text-muted)] mb-3 p-2 bg-[var(--bg-primary)] rounded">{selectedNode.tableData.comment}</p>
                )}
                
                <div className="space-y-2 text-xs">
                  <div className="flex justify-between py-1 border-b border-[var(--border)]">
                    <span className="text-[var(--text-muted)]">字段数</span>
                    <span className="font-mono">{selectedNode.tableData?.columns.length || 0}</span>
                  </div>
                  <div className="flex justify-between py-1 border-b border-[var(--border)]">
                    <span className="text-[var(--text-muted)]">外键数</span>
                    <span className="font-mono">{selectedNode.tableData?.foreign_keys.length || 0}</span>
                  </div>
                  <div className="flex justify-between py-1 border-b border-[var(--border)]">
                    <span className="text-[var(--text-muted)]">被引用数</span>
                    <span className="font-mono">{selectedNode.tableData?.referenced_by.length || 0}</span>
                  </div>
                </div>

                {/* 表结构列表 - 点击表始终显示 */}
                <div className="mt-4">
                  <h4 className="text-xs font-medium text-[var(--text-muted)] mb-2">表结构</h4>
                  <div className="space-y-1 max-h-72 overflow-y-auto">
                    {selectedNode.tableData?.columns.map((col: ColumnInfo) => (
                      <div 
                        key={col.name}
                        className={`flex items-center gap-2 px-2 py-1.5 rounded text-xs ${
                          col.key === 'PRI' ? 'bg-yellow-500/10 border border-yellow-500/30' : 'bg-[var(--bg-primary)]'
                        }`}
                      >
                        <span className="text-[var(--text-muted)] w-6 text-center font-mono text-[10px]">{getColumnTypeIcon(col.data_type)}</span>
                        <span className="flex-1 truncate font-mono">{col.name}</span>
                        <span className="text-[var(--text-muted)] text-[10px]">{col.data_type}</span>
                        {col.key === 'PRI' && <span className="text-yellow-500 text-[10px]">PK</span>}
                        {!col.nullable && <span className="text-red-400 text-[10px]">!</span>}
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            ) : (
              <div>
                <div className="flex items-center gap-2 mb-3">
                  <span className={`px-2 py-0.5 text-xs rounded ${
                    selectedNode.columnData?.key === 'PRI' ? 'bg-yellow-500/20 text-yellow-500' : 'bg-gray-500/20 text-gray-400'
                  }`}>COLUMN</span>
                  <h3 className="text-sm font-semibold text-[var(--text-primary)] truncate">{selectedNode.label}</h3>
                </div>
                
                <div className="text-xs text-[var(--text-muted)] mb-3">所属表: <span className="text-blue-400">{selectedNode.parentTable}</span></div>
                
                <div className="space-y-2">
                  <div className="flex justify-between py-1.5 border-b border-[var(--border)]">
                    <span className="text-[var(--text-muted)]">数据类型</span>
                    <span className="font-mono text-purple-400">{selectedNode.columnData?.data_type}</span>
                  </div>
                  <div className="flex justify-between py-1.5 border-b border-[var(--border)]">
                    <span className="text-[var(--text-muted)]">可空</span>
                    <span className={selectedNode.columnData?.nullable ? 'text-green-400' : 'text-red-400'}>
                      {selectedNode.columnData?.nullable ? 'YES' : 'NO'}
                    </span>
                  </div>
                  {selectedNode.columnData?.key && (
                    <div className="flex justify-between py-1.5 border-b border-[var(--border)]">
                      <span className="text-[var(--text-muted)]">键类型</span>
                      <span className="text-yellow-500 font-medium">{selectedNode.columnData.key}</span>
                    </div>
                  )}
                  {selectedNode.columnData?.comment && (
                    <div className="py-2">
                      <span className="text-[var(--text-muted)] block mb-1">备注</span>
                      <p className="text-xs text-[var(--text-secondary)] p-2 bg-[var(--bg-primary)] rounded">{selectedNode.columnData.comment}</p>
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* 确认弹窗 */}
      {confirmClear && (
        <ConfirmModal
          message="确定清除本地保存的可视化数据吗？此操作不可撤销。"
          onConfirm={clearLocalData}
          onCancel={() => setConfirmClear(false)}
          danger
        />
      )}
    </div>
  );
}
