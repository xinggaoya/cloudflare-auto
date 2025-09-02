class CloudflareManager {
    constructor() {
        this.selectedDomains = new Set();
        this.init();
    }

    async init() {
        this.bindEvents();
        await this.loadConfigStatus();
        await this.updateCurrentIp();
        await this.loadDnsUpdateRecords();
        setInterval(() => this.updateCurrentIp(), 30000); // 每30秒更新IP
    }

    bindEvents() {
        // 测试配置按钮
        document.getElementById('test-btn').addEventListener('click', () => {
            this.testConfig();
        });

        // 保存配置表单
        document.getElementById('cf-config-form').addEventListener('submit', (e) => {
            e.preventDefault();
            this.saveConfig();
        });

        // 全选按钮
        document.getElementById('select-all-btn').addEventListener('click', () => {
            this.selectAllDomains();
        });

        // 保存选择按钮
        document.getElementById('save-selection-btn').addEventListener('click', () => {
            this.saveDomainSelection();
        });

        // 立即更新按钮
        document.getElementById('manual-update-btn').addEventListener('click', () => {
            this.manualUpdate();
        });

        // 刷新记录按钮
        document.getElementById('refresh-records-btn').addEventListener('click', () => {
            this.loadDnsUpdateRecords();
        });
    }

    async testConfig() {
        const formData = this.getFormData();
        if (!this.validateForm(formData)) return;

        this.showLoading(true);
        
        try {
            const response = await fetch('/api/test-config', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(formData)
            });

            const result = await response.json();
            
            if (result.success) {
                this.showToast('配置测试成功！', 'success');
                await this.loadDomainList(formData);
            } else {
                this.showToast(result.message || '配置测试失败', 'error');
            }
        } catch (error) {
            this.showToast('网络错误: ' + error.message, 'error');
        } finally {
            this.showLoading(false);
        }
    }

    async loadDomainList(formData) {
        try {
            const response = await fetch('/api/domain-list', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(formData)
            });

            const result = await response.json();
            
            if (result.success) {
                this.displayDomainList(result.data.domains, result.data.current_ip);
                document.getElementById('domain-selection').style.display = 'block';
            } else {
                this.showToast(result.message || '获取域名列表失败', 'error');
            }
        } catch (error) {
            this.showToast('网络错误: ' + error.message, 'error');
        }
    }

    displayDomainList(domains, currentIp) {
        const domainListDiv = document.getElementById('domain-list');
        
        if (domains.length === 0) {
            domainListDiv.innerHTML = '<p>没有找到子域名，将使用根域名</p>';
            return;
        }

        let html = `<div class="domain-list">
            <div class="domain-item">
                <input type="checkbox" id="root-domain" value="" checked>
                <label for="root-domain">${document.getElementById('root-domain').value} (根域名)</label>
            </div>`;

        domains.forEach(domain => {
            const fullDomain = `${domain}.${document.getElementById('root-domain').value}`;
            html += `
            <div class="domain-item">
                <input type="checkbox" id="domain-${domain}" value="${domain}">
                <label for="domain-${domain}">${fullDomain}</label>
            </div>`;
        });

        html += '</div>';
        domainListDiv.innerHTML = html;

        // 绑定复选框事件
        document.querySelectorAll('.domain-item input[type="checkbox"]').forEach(checkbox => {
            checkbox.addEventListener('change', (e) => {
                if (e.target.checked) {
                    this.selectedDomains.add(e.target.value);
                } else {
                    this.selectedDomains.delete(e.target.value);
                }
            });
        });

        // 默认选择所有
        this.selectAllDomains();
    }

    selectAllDomains() {
        document.querySelectorAll('.domain-item input[type="checkbox"]').forEach(checkbox => {
            checkbox.checked = true;
            this.selectedDomains.add(checkbox.value);
        });
    }

    async saveDomainSelection() {
        const formData = this.getFormData();
        formData.selected_subdomains = Array.from(this.selectedDomains);
        formData.check_interval = parseInt(document.getElementById('check-interval').value) || 300;

        try {
            const response = await fetch('/api/save-config', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(formData)
            });

            const result = await response.json();
            
            if (result.success) {
                this.showToast('配置保存成功！', 'success');
                await this.loadConfigStatus();
            } else {
                this.showToast(result.message || '保存失败', 'error');
            }
        } catch (error) {
            this.showToast('网络错误: ' + error.message, 'error');
        }
    }

    async saveConfig() {
        const formData = this.getFormData();
        if (!this.validateForm(formData)) return;

        formData.selected_subdomains = Array.from(this.selectedDomains);
        formData.check_interval = parseInt(document.getElementById('check-interval').value) || 300;

        try {
            const response = await fetch('/api/save-config', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(formData)
            });

            const result = await response.json();
            
            if (result.success) {
                this.showToast('配置保存并更新成功！', 'success');
                await this.loadConfigStatus();
                await this.loadDnsUpdateRecords(); // 刷新更新记录
            } else {
                this.showToast(result.message || '保存失败', 'error');
            }
        } catch (error) {
            this.showToast('网络错误: ' + error.message, 'error');
        }
    }

    async loadConfigStatus() {
        try {
            const response = await fetch('/api/config-status');
            const result = await response.json();
            
            if (result.success) {
                this.displayConfigStatus(result.data);
            }
        } catch (error) {
            console.error('获取配置状态失败:', error);
        }
    }

    displayConfigStatus(status) {
        const statusDiv = document.getElementById('status-content');
        
        if (status.configured && status.current_config) {
            const config = status.current_config;
            statusDiv.innerHTML = `
                <div style="color: #48bb78; margin-bottom: 10px;">
                    ✅ 配置已保存
                </div>
                <div style="font-size: 0.9em; color: #666;">
                    <div>根域名: ${config.root_domain}</div>
                    <div>已选子域名: ${config.selected_subdomains.join(', ') || '无'}</div>
                    <div>检查间隔: ${config.check_interval}秒</div>
                </div>
            `;
            
            // 填充表单
            document.getElementById('api-key').value = config.cloudflare_api_key;
            document.getElementById('zone-id').value = config.cloudflare_zone_id;
            document.getElementById('root-domain').value = config.root_domain;
            document.getElementById('check-interval').value = config.check_interval;
            
            // 更新监控状态
            document.getElementById('monitor-status').textContent = '运行中';
            document.getElementById('monitor-status').style.color = '#48bb78';
            
        } else {
            statusDiv.innerHTML = '<div style="color: #e53e3e;">❌ 未配置</div>';
            document.getElementById('monitor-status').textContent = '未启动';
            document.getElementById('monitor-status').style.color = '#e53e3e';
        }
    }

    async updateCurrentIp() {
        try {
            const response = await fetch('/api/current-ip');
            const result = await response.json();
            
            if (result.success) {
                document.getElementById('current-ip').textContent = result.data;
            }
        } catch (error) {
            console.error('获取当前IP失败:', error);
        }
    }

    async manualUpdate() {
        this.showToast('开始手动更新...', 'info');
        
        try {
            // 这里需要实现手动更新逻辑
            this.showToast('手动更新功能待实现', 'info');
        } catch (error) {
            this.showToast('更新失败: ' + error.message, 'error');
        }
    }

    getFormData() {
        return {
            api_key: document.getElementById('api-key').value,
            zone_id: document.getElementById('zone-id').value,
            root_domain: document.getElementById('root-domain').value
        };
    }

    validateForm(data) {
        if (!data.api_key) {
            this.showToast('请输入API密钥', 'error');
            return false;
        }
        if (!data.zone_id) {
            this.showToast('请输入区域ID', 'error');
            return false;
        }
        if (!data.root_domain) {
            this.showToast('请输入根域名', 'error');
            return false;
        }
        return true;
    }

    showToast(message, type = 'info') {
        const toast = document.getElementById('toast');
        toast.textContent = message;
        toast.className = `toast ${type} show`;
        
        setTimeout(() => {
            toast.className = 'toast';
        }, 3000);
    }

    showLoading(show) {
        const buttons = document.querySelectorAll('button');
        buttons.forEach(btn => {
            if (show) {
                btn.classList.add('loading');
            } else {
                btn.classList.remove('loading');
            }
        });
    }

    async loadDnsUpdateRecords() {
        const recordsContent = document.getElementById('records-content');
        recordsContent.innerHTML = '<p>正在加载更新记录...</p>';
        
        try {
            const response = await fetch('/api/dns-update-records');
            const result = await response.json();
            
            if (result.success) {
                this.displayDnsUpdateRecords(result.data.records);
            } else {
                recordsContent.innerHTML = '<p style="color: #e53e3e;">加载失败: ' + (result.message || '未知错误') + '</p>';
            }
        } catch (error) {
            recordsContent.innerHTML = '<p style="color: #e53e3e;">网络错误: ' + error.message + '</p>';
        }
    }

    displayDnsUpdateRecords(records) {
        const recordsContent = document.getElementById('records-content');
        
        if (records.length === 0) {
            recordsContent.innerHTML = '<p>暂无更新记录</p>';
            return;
        }

        let html = '<div class="records-list">';
        
        records.forEach(record => {
            const timestamp = new Date(record.timestamp).toLocaleString('zh-CN');
            const successRate = record.domain_count > 0 
                ? Math.round((record.success_count / record.domain_count) * 100) 
                : 0;
            const statusClass = record.success_count === record.domain_count ? 'success' : 
                              record.success_count > 0 ? 'warning' : 'error';
            
            html += `
                <div class="record-item">
                    <div class="record-header">
                        <span class="record-time">${timestamp}</span>
                        <span class="record-status ${statusClass}">
                            ${record.success_count}/${record.domain_count} (${successRate}%)
                        </span>
                    </div>
                    <div class="record-details">
                        <div class="record-ip">
                            <span class="label">IP变化:</span>
                            <span class="value">${record.old_ip || '无'} → ${record.new_ip}</span>
                        </div>
                        ${record.error_message ? `
                        <div class="record-error">
                            <span class="label">错误:</span>
                            <span class="value">${record.error_message}</span>
                        </div>
                        ` : ''}
                    </div>
                </div>
            `;
        });
        
        html += '</div>';
        recordsContent.innerHTML = html;
    }
}

// 初始化应用
document.addEventListener('DOMContentLoaded', () => {
    new CloudflareManager();
});