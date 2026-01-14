// .agent/skills/oceanload-architect/templates/mui-reference/DownloadTable.jsx

import * as React from 'react';
import { DataGrid, GridToolbar } from '@mui/x-data-grid';
import Paper from '@mui/material/Paper';

// Adaptation: Columns optimized for Download Manager
const columns = [
    { field: 'id', headerName: 'ID', width: 70 },
    { field: 'name', headerName: 'Name', width: 200, flex: 1 },
    {
        field: 'progress',
        headerName: 'Progress',
        width: 150,
        renderCell: (params) => (
            // Agent Note: Insert LinearProgress here using Primary Teal color
            <span>{params.value}%</span>
        )
    },
    { field: 'speed', headerName: 'Speed', width: 130 },
    { field: 'status', headerName: 'Status', width: 120 },
];

export default function DownloadTable({ rows }) {
    return (
        <Paper sx={{ height: 400, width: '100%' }}>
            <DataGrid
                rows={rows}
                columns={columns}
                initialState={{ pagination: { paginationModel: { page: 0, pageSize: 10 } } }}
                pageSizeOptions={[5, 10]}
                checkboxSelection
                slots={{ toolbar: GridToolbar }} // Uses the standard CRUD toolbar
                sx={{ border: 0 }}
            />
        </Paper>
    );
}