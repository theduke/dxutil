use super::*;
#[component]
pub(super) fn TableEdges(ui: EditorUiState, handle: EngineHandle) -> Element {
    let Some(table) = ui.table.clone() else {
        return rsx! {};
    };
    let Some(rect) = ui.geometry.table.or(ui.geometry.block) else {
        return rsx! {};
    };
    let placement = format!(
        "left:{}px;top:{}px;width:{}px;height:{}px",
        rect.x, rect.y, rect.width, rect.height
    );
    let rows = table.rows;
    let cols = table.cols;
    let bottom = handle.clone();
    let right = handle.clone();
    let keys = handle.clone();
    rsx! {div{class:"dxeditor-table-edges",role:"toolbar",onmounted:init_roving,onfocusin:roving_focus,onkeydown:move|event|menu_keys(event,keys.clone()),aria_label:"Table editing",style:"{placement}",
        if let Some(cell)=ui.geometry.cell {div{class:"dxeditor-table-cell-outline",style:format!("position:absolute;pointer-events:none;left:{}px;top:{}px;width:{}px;height:{}px;border:2px solid var(--dx-accent,#176b87);box-sizing:border-box",cell.x-rect.x,cell.y-rect.y,cell.width,cell.height)}}
        for row in 0..rows{{let h=handle.clone();let uid=table.uid.clone();let y=ui.geometry.table_rows.get(row).map(|r|r.y-rect.y+r.height/2.).unwrap_or((row as f64+0.5)*rect.height/rows.max(1) as f64)-14.;rsx!{button{r#type:"button",class:"dxeditor-engine__button dxeditor-table-handle dxeditor-table-handle--row",aria_label:"Row {row+1} actions",aria_haspopup:"menu",draggable:true,"data-table-row-drag":"{row}","data-table-uid":"{table.uid}",style:"left:-30px;top:{y}px",onmousedown:move|e|e.prevent_default(),onclick:move |_|{h.run(Command::SetSelection(EditorSelection::Cells{table:uid.clone(),anchor:(row,0),head:(row,cols.saturating_sub(1))}));h.run(Command::OpenMenu("table-row".into()));},"⠿"}}}}
        for col in 0..cols{{let h=handle.clone();let uid=table.uid.clone();let x=ui.geometry.table_columns.get(col).map(|r|r.x-rect.x+r.width/2.).unwrap_or((col as f64+0.5)*rect.width/cols.max(1) as f64)-14.;rsx!{button{r#type:"button",class:"dxeditor-engine__button dxeditor-table-handle dxeditor-table-handle--column",aria_label:"Column {col+1} actions",aria_haspopup:"menu",style:"left:{x}px;top:-30px",onmousedown:move|e|e.prevent_default(),onclick:move |_|{h.run(Command::SetSelection(EditorSelection::Cells{table:uid.clone(),anchor:(0,col),head:(rows.saturating_sub(1),col)}));h.run(Command::OpenMenu("table-column".into()));},"⠿"}}}}
        button{r#type:"button",class:"dxeditor-engine__button dxeditor-table-rail dxeditor-table-rail--row",aria_label:"Add row at bottom",style:"left:0;top:{rect.height}px;width:{rect.width}px",onmousedown:move|e|e.prevent_default(),onclick:move |_|bottom.run(Command::Table(TableCommand::InsertAxis{axis:Axis::Row,index:rows.saturating_sub(1),after:true})),"+"}
        button{r#type:"button",class:"dxeditor-engine__button dxeditor-table-rail dxeditor-table-rail--column",aria_label:"Add column at right",style:"left:{rect.width}px;top:0;height:{rect.height}px",onmousedown:move|e|e.prevent_default(),onclick:move |_|right.run(Command::Table(TableCommand::InsertAxis{axis:Axis::Column,index:cols.saturating_sub(1),after:true})),"+"}
    }}
}
#[component]
pub(super) fn TableMenu(ui: EditorUiState, handle: EngineHandle) -> Element {
    let Some(table) = ui.table.clone() else {
        return rsx! {};
    };
    let axis = if ui.open_menu.as_deref() == Some("table-column") {
        Axis::Column
    } else {
        Axis::Row
    };
    let index = if axis == Axis::Row {
        table.anchor.0
    } else {
        table.anchor.1
    };
    let placement = style(
        &ui,
        ui.geometry.table.or(ui.geometry.block),
        Size {
            width: 240.,
            height: 360.,
        },
        Side::Bottom,
    );
    let keys = handle.clone();
    rsx! {div{class:"dxeditor-engine__surface dxeditor-engine__table-menu",role:"menu",onmounted:focus_first,onfocusin:roving_focus,aria_label:"Table actions",style:"{placement}",onkeydown:move|event|menu_keys(event,keys.clone()),
        MenuAction{label:"Header row".to_string(),handle:handle.clone(),command:Command::Table(TableCommand::ToggleHeader(Axis::Row)),checked:Some(table.header_row)}
        if ui.profile.allows_colors(){MenuAction{label:"Header column".to_string(),handle:handle.clone(),command:Command::Table(TableCommand::ToggleHeader(Axis::Column)),checked:Some(table.header_column)}}
        if ui.profile.allows_colors(){details{summary{"Color"}for color in std::iter::once(None).chain(crate::profiles::block_document::COLOR_TOKENS.into_iter().map(|c|Some(c.to_string()))){MenuAction{label:color.as_ref().map(|c|format!("{c} background")).unwrap_or_else(||"Default color".into()),handle:handle.clone(),command:Command::Table(TableCommand::SetBackground(color))}}}}
        MenuAction{label:(if axis==Axis::Row{"Insert above"}else{"Insert left"}).to_string(),handle:handle.clone(),command:Command::Table(TableCommand::InsertAxis{axis,index,after:false})}
        MenuAction{label:(if axis==Axis::Row{"Insert below"}else{"Insert right"}).to_string(),handle:handle.clone(),command:Command::Table(TableCommand::InsertAxis{axis,index,after:true})}
        MenuAction{label:"Duplicate".to_string(),handle:handle.clone(),command:Command::Table(TableCommand::DuplicateAxis{axis,index})}
        if axis==Axis::Row&&index>0{MenuAction{label:"Move row up".to_string(),handle:handle.clone(),command:Command::Table(TableCommand::MoveRow{from:index,to:index-1})}}
        if axis==Axis::Row&&index+1<table.rows{MenuAction{label:"Move row down".to_string(),handle:handle.clone(),command:Command::Table(TableCommand::MoveRow{from:index,to:index+1})}}
        MenuAction{label:"Clear contents".to_string(),handle:handle.clone(),command:Command::Table(TableCommand::Clear)}
        details{summary{"Alignment"}for alignment in ["left","center","right"]{MenuAction{label:format!("Align {alignment}"),handle:handle.clone(),command:Command::Table(TableCommand::SetAlignment(alignment.into()))}}}
        MenuAction{label:(if axis==Axis::Row{"Delete row"}else{"Delete column"}).to_string(),handle:handle.clone(),command:Command::Table(TableCommand::DeleteAxis{axis,index}),destructive:true}
        MenuAction{label:"Delete table".to_string(),handle,command:Command::Table(TableCommand::Delete),destructive:true}
    }}
}
